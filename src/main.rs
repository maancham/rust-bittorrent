use std::env;

#[allow(dead_code)]
fn decode_bencoded_value(encoded_value: &str) -> serde_json::Value {
    match encoded_value.chars().next() {
        Some(ch) if ch.is_ascii_digit() => {
            let colon_index = encoded_value.find(':').unwrap();
            let number = encoded_value[..colon_index].parse::<usize>().unwrap();
            let string = &encoded_value[colon_index + 1..colon_index + 1 + number];
            serde_json::Value::String(string.to_string())
        }
        Some('i') => {
            let end_index = encoded_value.find('e').unwrap();
            let number = encoded_value[1..end_index].parse::<i64>().unwrap();
            serde_json::Value::Number(number.into())
        }
        _ => panic!("Unhandled encoded value: {}", encoded_value)
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
