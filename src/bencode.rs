use serde_json::{Map, Value};

pub fn decode(encoded_value: &str) -> Value {
    decode_value(encoded_value.as_bytes()).0
}

pub fn decode_value(bytes: &[u8]) -> (Value, usize) {
    match bytes.first() {
        Some(&b'i') => decode_integer(bytes),
        Some(&b'l') => decode_list(bytes),
        Some(&b'd') => decode_dict(bytes),
        Some(&ch) if ch.is_ascii_digit() => decode_string(bytes),
        _ => panic!("Unhandled encoded value"),
    }
}

fn decode_integer(bytes: &[u8]) -> (Value, usize) {
    let end = bytes.iter().position(|&b| b == b'e').unwrap();
    let number = std::str::from_utf8(&bytes[1..end])
        .unwrap()
        .parse::<i64>()
        .unwrap();
    (Value::Number(number.into()), end + 1)
}

fn decode_list(bytes: &[u8]) -> (Value, usize) {
    let mut values = Vec::new();
    let mut pos = 1;

    while bytes[pos] != b'e' {
        let (value, consumed) = decode_value(&bytes[pos..]);
        values.push(value);
        pos += consumed;
    }

    (Value::Array(values), pos + 1)
}

fn decode_dict(bytes: &[u8]) -> (Value, usize) {
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

fn decode_string(bytes: &[u8]) -> (Value, usize) {
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

pub fn extract_info_bytes(bytes: &[u8]) -> Option<&[u8]> {
    let info_key = b"4:info";
    bytes
        .windows(info_key.len())
        .position(|window| window == info_key)
        .map(|pos| {
            let start = pos + info_key.len();
            let (_, consumed) = decode_value(&bytes[start..]);
            &bytes[start..start + consumed]
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_integer() {
        let result = decode("i52e");
        assert_eq!(result.as_i64().unwrap(), 52);
    }

    #[test]
    fn test_decode_negative_integer() {
        let result = decode("i-52e");
        assert_eq!(result.as_i64().unwrap(), -52);
    }

    #[test]
    fn test_decode_string() {
        let result = decode("5:hello");
        assert_eq!(result.as_str().unwrap(), "hello");
    }

    #[test]
    fn test_decode_list() {
        let result = decode("l5:helloi52ee");
        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0].as_str().unwrap(), "hello");
        assert_eq!(arr[1].as_i64().unwrap(), 52);
    }

    #[test]
    fn test_decode_dict() {
        let result = decode("d3:foo3:bar5:helloi52ee");
        let obj = result.as_object().unwrap();
        assert_eq!(obj.get("foo").unwrap().as_str().unwrap(), "bar");
        assert_eq!(obj.get("hello").unwrap().as_i64().unwrap(), 52);
    }

    #[test]
    fn test_decode_nested_list() {
        let result = decode("lli1ei2ei3eee");
        let outer = result.as_array().unwrap();
        assert_eq!(outer.len(), 1);
        let inner = outer[0].as_array().unwrap();
        assert_eq!(inner.len(), 3);
        assert_eq!(inner[0].as_i64().unwrap(), 1);
    }
}
