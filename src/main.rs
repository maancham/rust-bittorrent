use std::env;
use serde_json::{Map, Value};

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
            let string = std::str::from_utf8(&bytes[start..start + length])
                .unwrap()
                .to_string();
            (Value::String(string), start + length)
        }
        _ => panic!("Unhandled encoded value")
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let command = &args[1];

    if command == "decode" {
        let encoded_value = &args[2];
        let decoded_value = decode_bencoded_value(encoded_value);
        println!("{}", decoded_value.to_string());
    } else {
        println!("unknown command: {}", args[1])
    }
}
