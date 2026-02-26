use std::fs;
use std::io::{Read, Write};
use std::net::TcpStream;

use log::info;

use crate::bencode::{decode_value, extract_info_bytes};
use crate::peer::{
    create_handshake, download_piece_blocks, generate_peer_id, parse_peers, perform_handshake,
    send_interested, wait_for_bitfield, wait_for_unchoke,
};
use crate::utils::{calculate_hash, url_encode_bytes, verify_piece};

pub struct Torrent {
    pub announce: String,
    pub length: i64,
    pub info_hash: Vec<u8>,
    pub piece_length: i64,
    pub piece_hashes: Vec<String>,
}

impl Torrent {
    pub fn from_file(path: &str) -> Self {
        let bytes = fs::read(path).unwrap();
        let decoded = decode_value(&bytes).0;

        let torrent_dict = decoded.as_object().unwrap();
        let announce = torrent_dict
            .get("announce")
            .and_then(|v| v.as_str())
            .unwrap()
            .to_string();

        let info_bytes = extract_info_bytes(&bytes).unwrap();
        Self::from_info_bytes(&announce, info_bytes)
    }

    pub fn from_info_bytes(announce: &str, info_bytes: &[u8]) -> Self {
        let info = decode_value(info_bytes).0;
        let info_dict = info.as_object().unwrap();

        let length = info_dict.get("length").and_then(|v| v.as_i64()).unwrap();
        let piece_length = info_dict
            .get("piece length")
            .and_then(|v| v.as_i64())
            .unwrap();
        let info_hash = calculate_hash(info_bytes);

        let pieces_hex = info_dict.get("pieces").and_then(|v| v.as_str()).unwrap();
        let piece_hashes: Vec<String> = hex::decode(pieces_hex)
            .unwrap()
            .chunks(20)
            .map(hex::encode)
            .collect();

        Self {
            announce: announce.to_string(),
            length,
            info_hash,
            piece_length,
            piece_hashes,
        }
    }

    pub fn info_hash_hex(&self) -> String {
        hex::encode(&self.info_hash)
    }

    pub fn discover_peers(&self) -> Vec<String> {
        let peer_id = generate_peer_id();
        let url = format!(
            "{}?info_hash={}&peer_id={}&port=6881&uploaded=0&downloaded=0&left={}&compact=1",
            self.announce,
            url_encode_bytes(&self.info_hash),
            url_encode_bytes(&peer_id),
            self.length
        );

        let response = reqwest::blocking::get(&url).unwrap();
        let response_bytes = response.bytes().unwrap();
        let decoded = decode_value(&response_bytes).0;

        let peers_hex = decoded
            .as_object()
            .unwrap()
            .get("peers")
            .and_then(|v| v.as_str())
            .unwrap();

        let peers_bytes = hex::decode(peers_hex).unwrap();
        let peers = parse_peers(&peers_bytes);
        info!("Discovered {} peers", peers.len());
        peers
    }

    pub fn handshake(&self, peer_addr: &str) -> String {
        let peer_id = generate_peer_id();
        let handshake_msg = create_handshake(&self.info_hash, &peer_id);

        let mut stream = TcpStream::connect(peer_addr).unwrap();
        stream.write_all(&handshake_msg).unwrap();

        let mut response = [0u8; 68];
        stream.read_exact(&mut response).unwrap();

        hex::encode(&response[48..68])
    }

    pub fn download_piece(&self, piece_index: usize, output_path: &str) {
        let peers = self.discover_peers();
        let peer_addr = &peers[0];

        let peer_id = generate_peer_id();
        let mut stream = TcpStream::connect(peer_addr).unwrap();

        perform_handshake(&mut stream, &self.info_hash, &peer_id);

        wait_for_bitfield(&mut stream);
        send_interested(&mut stream);
        wait_for_unchoke(&mut stream);

        let piece_data = download_piece_blocks(
            &mut stream,
            piece_index,
            self.piece_length as usize,
            self.length as usize,
        );

        verify_piece(&piece_data, &self.piece_hashes[piece_index]);
        fs::write(output_path, piece_data).unwrap();
    }

    pub fn download(&self, output_path: &str) {
        info!("Starting download of {} pieces", self.piece_hashes.len());
        let peers = self.discover_peers();
        let peer_addr = &peers[0];

        let peer_id = generate_peer_id();
        let mut stream = TcpStream::connect(peer_addr).unwrap();

        perform_handshake(&mut stream, &self.info_hash, &peer_id);

        wait_for_bitfield(&mut stream);
        send_interested(&mut stream);
        wait_for_unchoke(&mut stream);

        let num_pieces = self.piece_hashes.len();
        let mut file_data = Vec::with_capacity(self.length as usize);

        for piece_index in 0..num_pieces {
            let piece_data = download_piece_blocks(
                &mut stream,
                piece_index,
                self.piece_length as usize,
                self.length as usize,
            );

            verify_piece(&piece_data, &self.piece_hashes[piece_index]);
            file_data.extend_from_slice(&piece_data);

            info!("Downloaded piece {}/{}", piece_index + 1, num_pieces);
        }

        fs::write(output_path, file_data).unwrap();
        info!("Download complete");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_info_hash_hex() {
        let torrent = Torrent {
            announce: "test".to_string(),
            length: 100,
            info_hash: vec![0x12, 0x34, 0x56, 0x78],
            piece_length: 1024,
            piece_hashes: vec![],
        };

        assert_eq!(torrent.info_hash_hex(), "12345678");
    }

    #[test]
    fn test_from_info_bytes() {
        let piece_hash = [0xabu8; 20];
        let expected_hash_hex = hex::encode(piece_hash);

        let mut info_bytes = Vec::new();
        info_bytes.extend_from_slice(b"d6:lengthi1234e12:piece lengthi512e6:pieces20:");
        info_bytes.extend_from_slice(&piece_hash);
        info_bytes.push(b'e');

        let torrent = Torrent::from_info_bytes("http://tracker.example.com/announce", &info_bytes);

        assert_eq!(torrent.announce, "http://tracker.example.com/announce");
        assert_eq!(torrent.length, 1234);
        assert_eq!(torrent.piece_length, 512);
        assert_eq!(torrent.piece_hashes.len(), 1);
        assert_eq!(torrent.piece_hashes[0], expected_hash_hex);
    }

    #[test]
    #[should_panic]
    fn test_from_info_bytes_missing_length() {
        let info_bencoded = b"d12:piece lengthi512ee";
        Torrent::from_info_bytes("http://tracker.example.com/announce", info_bencoded);
    }
}
