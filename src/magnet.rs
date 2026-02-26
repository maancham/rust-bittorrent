use crate::bencode;
use crate::peer::{
    download_piece_blocks, generate_peer_id, parse_peers, perform_handshake_with_extensions,
    receive_extension_handshake, receive_metadata_piece, send_extension_handshake, send_interested,
    send_metadata_request, wait_for_bitfield, wait_for_unchoke,
};
use crate::torrent::Torrent;
use crate::utils::{url_encode_bytes, verify_piece};
use std::collections::HashMap;
use std::fs;
use std::net::TcpStream;

pub struct MagnetLink {
    pub tracker_url: Option<String>,
    pub info_hash: String,
    #[allow(dead_code)]
    pub name: Option<String>,
}

impl MagnetLink {
    pub fn parse(magnet_url: &str) -> Self {
        if !magnet_url.starts_with("magnet:?") {
            panic!("Invalid magnet link format");
        }

        let query_string = &magnet_url[8..];
        let params = parse_query_params(query_string);

        let info_hash = params
            .get("xt")
            .and_then(|xt| extract_info_hash(xt))
            .expect("Info hash (xt) is required");

        let tracker_url = params.get("tr").map(|tr| url_decode(tr));
        let name = params.get("dn").map(|dn| url_decode(dn));

        Self { tracker_url, info_hash, name }
    }

    pub fn info_hash_bytes(&self) -> Vec<u8> {
        hex::decode(&self.info_hash).expect("Invalid info hash")
    }

    pub fn discover_peers(&self) -> Vec<String> {
        let tracker_url = self
            .tracker_url
            .as_ref()
            .expect("Tracker URL required for peer discovery");

        let info_hash_bytes = self.info_hash_bytes();
        let peer_id = generate_peer_id();

        let url = format!(
            "{}?info_hash={}&peer_id={}&port=6881&uploaded=0&downloaded=0&left=999&compact=1",
            tracker_url,
            url_encode_bytes(&info_hash_bytes),
            url_encode_bytes(&peer_id),
        );

        let response = reqwest::blocking::get(&url).unwrap();
        let response_bytes = response.bytes().unwrap();

        let decoded = bencode::decode_value(&response_bytes).0;

        let peers_hex = decoded
            .as_object()
            .and_then(|obj| obj.get("peers"))
            .and_then(|v| v.as_str())
            .expect("Failed to get peers from tracker response");

        let peers_bytes = hex::decode(peers_hex).unwrap();
        parse_peers(&peers_bytes)
    }

    pub fn info(&self, peer_addr: &str) -> Torrent {
        self.fetch_torrent_with_stream(peer_addr).0
    }

    pub fn download_piece(&self, piece_index: usize, output_path: &str) {
        let peers = self.discover_peers();
        let (torrent, mut stream) = self.fetch_torrent_with_stream(&peers[0]);

        send_interested(&mut stream);
        wait_for_unchoke(&mut stream);

        let piece_data = download_piece_blocks(
            &mut stream,
            piece_index,
            torrent.piece_length as usize,
            torrent.length as usize,
        );

        verify_piece(&piece_data, &torrent.piece_hashes[piece_index]);
        fs::write(output_path, piece_data).unwrap();
    }

    fn fetch_torrent_with_stream(&self, peer_addr: &str) -> (Torrent, TcpStream) {
        let info_hash_bytes = self.info_hash_bytes();
        let peer_id = generate_peer_id();

        let mut stream = TcpStream::connect(peer_addr).unwrap();

        let (_, peer_supports_extensions) =
            perform_handshake_with_extensions(&mut stream, &info_hash_bytes, &peer_id);

        wait_for_bitfield(&mut stream);

        assert!(peer_supports_extensions, "Peer does not support extensions");

        send_extension_handshake(&mut stream);
        let ut_metadata_id = receive_extension_handshake(&mut stream);
        send_metadata_request(&mut stream, ut_metadata_id);
        let metadata = receive_metadata_piece(&mut stream);

        let announce = self.tracker_url.as_deref().unwrap_or("");
        (Torrent::from_info_bytes(announce, &metadata), stream)
    }

    pub fn handshake(&self, peer_addr: &str) -> (String, Option<u64>) {
        let info_hash_bytes = self.info_hash_bytes();
        let peer_id = generate_peer_id();

        let mut stream = TcpStream::connect(peer_addr).unwrap();

        let (peer_id_hex, peer_supports_extensions) =
            perform_handshake_with_extensions(&mut stream, &info_hash_bytes, &peer_id);

        wait_for_bitfield(&mut stream);

        let metadata_ext_id = if peer_supports_extensions {
            send_extension_handshake(&mut stream);
            Some(receive_extension_handshake(&mut stream))
        } else {
            None
        };

        (peer_id_hex, metadata_ext_id)
    }
}

fn parse_query_params(query: &str) -> HashMap<String, String> {
    query
        .split('&')
        .filter_map(|param| {
            let mut parts = param.splitn(2, '=');
            let key = parts.next()?;
            let value = parts.next()?;
            Some((key.to_string(), value.to_string()))
        })
        .collect()
}

fn extract_info_hash(xt: &str) -> Option<String> {
    xt.strip_prefix("urn:btih:").map(|hash| hash.to_string())
}

fn url_decode(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            '%' => {
                let hex: String = chars.by_ref().take(2).collect();
                if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                    result.push(byte as char);
                } else {
                    result.push('%');
                    result.push_str(&hex);
                }
            }
            '+' => result.push(' '),
            _ => result.push(ch),
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_magnet_link_with_all_params() {
        let link = "magnet:?xt=urn:btih:ad42ce8109f54c99613ce38f9b4d87e70f24a165&dn=magnet1.gif&tr=http%3A%2F%2Fbittorrent-test-tracker.codecrafters.io%2Fannounce";
        let magnet = MagnetLink::parse(link);

        assert_eq!(magnet.info_hash, "ad42ce8109f54c99613ce38f9b4d87e70f24a165");
        assert_eq!(
            magnet.tracker_url,
            Some("http://bittorrent-test-tracker.codecrafters.io/announce".to_string())
        );
        assert_eq!(magnet.name, Some("magnet1.gif".to_string()));
    }

    #[test]
    fn test_parse_magnet_link_minimal() {
        let link = "magnet:?xt=urn:btih:d69f91e6b2ae4c542468d1073a71d4ea13879a7f";
        let magnet = MagnetLink::parse(link);

        assert_eq!(magnet.info_hash, "d69f91e6b2ae4c542468d1073a71d4ea13879a7f");
        assert_eq!(magnet.tracker_url, None);
        assert_eq!(magnet.name, None);
    }

    #[test]
    fn test_url_decode() {
        assert_eq!(
            url_decode("http%3A%2F%2Fexample.com%2Fannounce"),
            "http://example.com/announce"
        );
        assert_eq!(url_decode("hello+world"), "hello world");
        assert_eq!(url_decode("no%20encoding"), "no encoding");
    }

    #[test]
    fn test_parse_query_params() {
        let params = parse_query_params("key1=value1&key2=value2&key3=value3");
        assert_eq!(params.get("key1"), Some(&"value1".to_string()));
        assert_eq!(params.get("key2"), Some(&"value2".to_string()));
        assert_eq!(params.get("key3"), Some(&"value3".to_string()));
    }

    #[test]
    fn test_extract_info_hash() {
        let hash = extract_info_hash("urn:btih:ad42ce8109f54c99613ce38f9b4d87e70f24a165");
        assert_eq!(hash, Some("ad42ce8109f54c99613ce38f9b4d87e70f24a165".to_string()));

        let invalid = extract_info_hash("invalid");
        assert_eq!(invalid, None);
    }

    #[test]
    #[should_panic(expected = "Invalid magnet link format")]
    fn test_invalid_magnet_format() {
        MagnetLink::parse("not_a_magnet_link");
    }

    #[test]
    #[should_panic(expected = "Info hash (xt) is required")]
    fn test_missing_info_hash() {
        MagnetLink::parse("magnet:?dn=test.txt&tr=http://tracker.com");
    }

    #[test]
    fn test_info_hash_bytes() {
        let magnet = MagnetLink {
            tracker_url: None,
            info_hash: "ad42ce8109f54c99613ce38f9b4d87e70f24a165".to_string(),
            name: None,
        };

        let bytes = magnet.info_hash_bytes();
        assert_eq!(bytes.len(), 20);
        assert_eq!(hex::encode(bytes), "ad42ce8109f54c99613ce38f9b4d87e70f24a165");
    }
}
