use std::{env, fs};
use serde_json::{Map, Value};
use sha1::{Digest, Sha1};

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

fn calculate_info_hash(info_bytes: &[u8]) -> String {
    let mut hasher = Sha1::new();
    hasher.update(info_bytes);
    hex::encode(hasher.finalize())
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
            let file_path = &args[2];
            let bytes = fs::read(file_path).unwrap();
            let decoded = decode_value(&bytes).0;
            
            let torrent = decoded.as_object().unwrap();
            let tracker_url = torrent.get("announce").and_then(|v| v.as_str()).unwrap();
            let info = torrent.get("info").and_then(|v| v.as_object()).unwrap();
            let length = info.get("length").and_then(|v| v.as_i64()).unwrap();
            let piece_length = info.get("piece length").and_then(|v| v.as_i64()).unwrap();
            let pieces_hex = info.get("pieces").and_then(|v| v.as_str()).unwrap();
            
            let info_bytes = extract_info_bytes(&bytes).unwrap();
            let info_hash = calculate_info_hash(info_bytes);
            
            let piece_hashes: Vec<String> = hex::decode(pieces_hex)
                .unwrap()
                .chunks(20)
                .map(hex::encode)
                .collect();
            
            println!("Tracker URL: {}", tracker_url);
            println!("Length: {}", length);
            println!("Info Hash: {}", info_hash);
            println!("Piece Length: {}", piece_length);
            println!("Piece Hashes:");
            piece_hashes.iter().for_each(|hash| println!("{}", hash));
        }
        _ => println!("unknown command: {}", command)
    }
}
