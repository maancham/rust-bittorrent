use sha1::{Digest, Sha1};

pub fn calculate_hash(bytes: &[u8]) -> Vec<u8> {
    let mut hasher = Sha1::new();
    hasher.update(bytes);
    hasher.finalize().to_vec()
}

pub fn url_encode_bytes(bytes: &[u8]) -> String {
    bytes.iter().map(|&b| format!("%{:02x}", b)).collect()
}

pub fn verify_piece(piece_data: &[u8], expected_hash: &str) {
    let actual_hash = hex::encode(calculate_hash(piece_data));
    assert_eq!(actual_hash, expected_hash, "Piece hash mismatch");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_hash() {
        let data = b"hello world";
        let hash = calculate_hash(data);
        assert_eq!(hash.len(), 20);
        assert_eq!(hex::encode(hash), "2aae6c35c94fcfb415dbe95f408b9ce91ee846ed");
    }

    #[test]
    fn test_url_encode_bytes() {
        let bytes = vec![0x12, 0x34, 0xAB, 0xCD];
        let encoded = url_encode_bytes(&bytes);
        assert_eq!(encoded, "%12%34%ab%cd");
    }

    #[test]
    fn test_verify_piece_success() {
        let data = b"hello world";
        let expected = "2aae6c35c94fcfb415dbe95f408b9ce91ee846ed";
        verify_piece(data, expected);
    }

    #[test]
    #[should_panic(expected = "Piece hash mismatch")]
    fn test_verify_piece_failure() {
        let data = b"hello world";
        let wrong_hash = "0000000000000000000000000000000000000000";
        verify_piece(data, wrong_hash);
    }
}
