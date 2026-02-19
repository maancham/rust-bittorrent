use std::{env, fs, time::SystemTime, io::{Read, Write}, net::TcpStream};
use serde_json::{Map, Value};
use sha1::{Digest, Sha1};

struct Torrent {
    announce: String,
    length: i64,
    info_hash: Vec<u8>,
    piece_length: i64,
    piece_hashes: Vec<String>,
}

impl Torrent {
    fn from_file(path: &str) -> Self {
        let bytes = fs::read(path).unwrap();
        let decoded = decode_value(&bytes).0;
        
        let torrent_dict = decoded.as_object().unwrap();
        let info = torrent_dict.get("info").and_then(|v| v.as_object()).unwrap();
        
        let announce = torrent_dict
            .get("announce")
            .and_then(|v| v.as_str())
            .unwrap()
            .to_string();
        
        let length = info.get("length").and_then(|v| v.as_i64()).unwrap();
        let piece_length = info.get("piece length").and_then(|v| v.as_i64()).unwrap();
        
        let info_bytes = extract_info_bytes(&bytes).unwrap();
        let info_hash = calculate_hash(info_bytes);
        
        let pieces_hex = info.get("pieces").and_then(|v| v.as_str()).unwrap();
        let piece_hashes = hex::decode(pieces_hex)
            .unwrap()
            .chunks(20)
            .map(hex::encode)
            .collect();
        
        Self {
            announce,
            length,
            info_hash,
            piece_length,
            piece_hashes,
        }
    }
    
    fn info_hash_hex(&self) -> String {
        hex::encode(&self.info_hash)
    }
    
    fn discover_peers(&self) -> Vec<String> {
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
        parse_peers(&peers_bytes)
    }
    
    fn handshake(&self, peer_addr: &str) -> String {
        let peer_id = generate_peer_id();
        let handshake_msg = create_handshake(&self.info_hash, &peer_id);
        
        let mut stream = TcpStream::connect(peer_addr).unwrap();
        stream.write_all(&handshake_msg).unwrap();
        
        let mut response = [0u8; 68];
        stream.read_exact(&mut response).unwrap();
        
        hex::encode(&response[48..68])
    }
    
    fn download_piece(&self, piece_index: usize, output_path: &str) {
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
}

fn decode_bencoded_value(encoded_value: &str) -> Value {
    decode_value(encoded_value.as_bytes()).0
}

fn decode_value(bytes: &[u8]) -> (Value, usize) {
    match bytes.first() {
        Some(&b'i') => {
            let end = bytes.iter().position(|&b| b == b'e').unwrap();
            let number = std::str::from_utf8(&bytes[1..end])
                .unwrap()
                .parse::<i64>()
                .unwrap();
            (Value::Number(number.into()), end + 1)
        }
        Some(&b'l') => {
            let mut values = Vec::new();
            let mut pos = 1;
            
            while bytes[pos] != b'e' {
                let (value, consumed) = decode_value(&bytes[pos..]);
                values.push(value);
                pos += consumed;
            }
            
            (Value::Array(values), pos + 1)
        }
        Some(&b'd') => {
            let mut map = Map::new();
            let mut pos = 1;
            
            while bytes[pos] != b'e' {
                let (key, key_consumed) = decode_value(&bytes[pos..]);
                let key_str = key.as_str().unwrap().to_string();
                pos += key_consumed;
                
                let (value, value_consumed) = decode_value(&bytes[pos..]);
                map.insert(key_str, value);
                pos += value_consumed;
            }
            
            (Value::Object(map), pos + 1)
        }
        Some(&ch) if ch.is_ascii_digit() => {
            let colon = bytes.iter().position(|&b| b == b':').unwrap();
            let length = std::str::from_utf8(&bytes[..colon])
                .unwrap()
                .parse::<usize>()
                .unwrap();
            let start = colon + 1;
            let end = start + length;
            
            let value = std::str::from_utf8(&bytes[start..end])
                .ok()
                .map(|s| Value::String(s.to_string()))
                .unwrap_or_else(|| Value::String(hex::encode(&bytes[start..end])));
            
            (value, end)
        }
        _ => panic!("Unhandled encoded value")
    }
}

fn extract_info_bytes(bytes: &[u8]) -> Option<&[u8]> {
    let info_key = b"4:info";
    bytes.windows(info_key.len())
        .position(|window| window == info_key)
        .map(|pos| {
            let start = pos + info_key.len();
            let (_, consumed) = decode_value(&bytes[start..]);
            &bytes[start..start + consumed]
        })
}

fn calculate_hash(bytes: &[u8]) -> Vec<u8> {
    let mut hasher = Sha1::new();
    hasher.update(bytes);
    hasher.finalize().to_vec()
}

fn url_encode_bytes(bytes: &[u8]) -> String {
    bytes.iter()
        .map(|&b| format!("%{:02x}", b))
        .collect()
}

fn parse_peers(peers_bytes: &[u8]) -> Vec<String> {
    peers_bytes.chunks(6)
        .map(|chunk| {
            let ip = format!("{}.{}.{}.{}", chunk[0], chunk[1], chunk[2], chunk[3]);
            let port = u16::from_be_bytes([chunk[4], chunk[5]]);
            format!("{}:{}", ip, port)
        })
        .collect()
}

fn generate_peer_id() -> [u8; 20] {
    let mut peer_id = [0u8; 20];
    peer_id[..8].copy_from_slice(b"-RS0001-");
    
    let timestamp = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    
    let timestamp_bytes = timestamp.to_le_bytes();
    peer_id[8..20].copy_from_slice(&timestamp_bytes[..12]);
    
    peer_id
}

fn create_handshake(info_hash: &[u8], peer_id: &[u8]) -> Vec<u8> {
    let mut handshake = Vec::with_capacity(68);
    
    handshake.push(19);
    handshake.extend_from_slice(b"BitTorrent protocol");
    handshake.extend_from_slice(&[0u8; 8]);
    handshake.extend_from_slice(info_hash);
    handshake.extend_from_slice(peer_id);
    
    handshake
}

fn perform_handshake(stream: &mut TcpStream, info_hash: &[u8], peer_id: &[u8]) {
    let handshake_msg = create_handshake(info_hash, peer_id);
    stream.write_all(&handshake_msg).unwrap();
    
    let mut response = [0u8; 68];
    stream.read_exact(&mut response).unwrap();
}

fn read_message(stream: &mut TcpStream) -> (u8, Vec<u8>) {
    let mut length_buf = [0u8; 4];
    stream.read_exact(&mut length_buf).unwrap();
    let length = u32::from_be_bytes(length_buf);
    
    if length == 0 {
        return (0, vec![]);
    }
    
    let mut message_id = [0u8; 1];
    stream.read_exact(&mut message_id).unwrap();
    
    let payload_length = length - 1;
    let mut payload = vec![0u8; payload_length as usize];
    if payload_length > 0 {
        stream.read_exact(&mut payload).unwrap();
    }
    
    (message_id[0], payload)
}

fn send_message(stream: &mut TcpStream, message_id: u8, payload: &[u8]) {
    let length = 1 + payload.len() as u32;
    stream.write_all(&length.to_be_bytes()).unwrap();
    stream.write_all(&[message_id]).unwrap();
    stream.write_all(payload).unwrap();
}

fn wait_for_bitfield(stream: &mut TcpStream) {
    let (msg_id, _) = read_message(stream);
    assert_eq!(msg_id, 5, "Expected bitfield message");
}

fn send_interested(stream: &mut TcpStream) {
    send_message(stream, 2, &[]);
}

fn wait_for_unchoke(stream: &mut TcpStream) {
    let (msg_id, _) = read_message(stream);
    assert_eq!(msg_id, 1, "Expected unchoke message");
}

fn send_request(stream: &mut TcpStream, index: u32, begin: u32, length: u32) {
    let mut payload = Vec::with_capacity(12);
    payload.extend_from_slice(&index.to_be_bytes());
    payload.extend_from_slice(&begin.to_be_bytes());
    payload.extend_from_slice(&length.to_be_bytes());
    send_message(stream, 6, &payload);
}

fn download_piece_blocks(
    stream: &mut TcpStream,
    piece_index: usize,
    piece_length: usize,
    total_length: usize,
) -> Vec<u8> {
    const BLOCK_SIZE: usize = 16 * 1024;
    
    let actual_piece_length = if (piece_index + 1) * piece_length > total_length {
        total_length - piece_index * piece_length
    } else {
        piece_length
    };
    
    let num_blocks = (actual_piece_length + BLOCK_SIZE - 1) / BLOCK_SIZE;
    
    for block_index in 0..num_blocks {
        let begin = block_index * BLOCK_SIZE;
        let block_length = if begin + BLOCK_SIZE > actual_piece_length {
            actual_piece_length - begin
        } else {
            BLOCK_SIZE
        };
        
        send_request(stream, piece_index as u32, begin as u32, block_length as u32);
    }
    
    let mut piece_data = vec![0u8; actual_piece_length];
    
    for _ in 0..num_blocks {
        let (msg_id, payload) = read_message(stream);
        assert_eq!(msg_id, 7, "Expected piece message");
        
        let begin = u32::from_be_bytes([payload[4], payload[5], payload[6], payload[7]]) as usize;
        let block_data = &payload[8..];
        
        piece_data[begin..begin + block_data.len()].copy_from_slice(block_data);
    }
    
    piece_data
}

fn verify_piece(piece_data: &[u8], expected_hash: &str) {
    let actual_hash = hex::encode(calculate_hash(piece_data));
    assert_eq!(actual_hash, expected_hash, "Piece hash mismatch");
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let command = &args[1];

    match command.as_str() {
        "decode" => {
            let encoded_value = &args[2];
            let decoded_value = decode_bencoded_value(encoded_value);
            println!("{}", decoded_value.to_string());
        }
        "info" => {
            let torrent = Torrent::from_file(&args[2]);
            
            println!("Tracker URL: {}", torrent.announce);
            println!("Length: {}", torrent.length);
            println!("Info Hash: {}", torrent.info_hash_hex());
            println!("Piece Length: {}", torrent.piece_length);
            println!("Piece Hashes:");
            torrent.piece_hashes.iter().for_each(|hash| println!("{}", hash));
        }
        "peers" => {
            let torrent = Torrent::from_file(&args[2]);
            let peers = torrent.discover_peers();
            peers.iter().for_each(|peer| println!("{}", peer));
        }
        "handshake" => {
            let torrent = Torrent::from_file(&args[2]);
            let peer_addr = &args[3];
            let peer_id = torrent.handshake(peer_addr);
            println!("Peer ID: {}", peer_id);
        }
        "download_piece" => {
            let output_flag = &args[2];
            assert_eq!(output_flag, "-o", "Expected -o flag");
            let output_path = &args[3];
            let torrent_file = &args[4];
            let piece_index = args[5].parse::<usize>().unwrap();
            
            let torrent = Torrent::from_file(torrent_file);
            torrent.download_piece(piece_index, output_path);
            println!("Piece {} downloaded to {}.", piece_index, output_path);
        }
        _ => println!("unknown command: {}", command)
    }
}
