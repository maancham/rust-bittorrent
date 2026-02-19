use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::SystemTime;

pub fn generate_peer_id() -> [u8; 20] {
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

pub fn create_handshake(info_hash: &[u8], peer_id: &[u8]) -> Vec<u8> {
    let mut handshake = Vec::with_capacity(68);

    handshake.push(19);
    handshake.extend_from_slice(b"BitTorrent protocol");
    handshake.extend_from_slice(&[0u8; 8]);
    handshake.extend_from_slice(info_hash);
    handshake.extend_from_slice(peer_id);

    handshake
}

pub fn perform_handshake(stream: &mut TcpStream, info_hash: &[u8], peer_id: &[u8]) {
    let handshake_msg = create_handshake(info_hash, peer_id);
    stream.write_all(&handshake_msg).unwrap();

    let mut response = [0u8; 68];
    stream.read_exact(&mut response).unwrap();
}

pub fn read_message(stream: &mut TcpStream) -> (u8, Vec<u8>) {
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

pub fn send_message(stream: &mut TcpStream, message_id: u8, payload: &[u8]) {
    let length = 1 + payload.len() as u32;
    stream.write_all(&length.to_be_bytes()).unwrap();
    stream.write_all(&[message_id]).unwrap();
    stream.write_all(payload).unwrap();
}

pub fn wait_for_bitfield(stream: &mut TcpStream) {
    let (msg_id, _) = read_message(stream);
    assert_eq!(msg_id, 5, "Expected bitfield message");
}

pub fn send_interested(stream: &mut TcpStream) {
    send_message(stream, 2, &[]);
}

pub fn wait_for_unchoke(stream: &mut TcpStream) {
    let (msg_id, _) = read_message(stream);
    assert_eq!(msg_id, 1, "Expected unchoke message");
}

pub fn send_request(stream: &mut TcpStream, index: u32, begin: u32, length: u32) {
    let mut payload = Vec::with_capacity(12);
    payload.extend_from_slice(&index.to_be_bytes());
    payload.extend_from_slice(&begin.to_be_bytes());
    payload.extend_from_slice(&length.to_be_bytes());
    send_message(stream, 6, &payload);
}

pub fn download_piece_blocks(
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

pub fn parse_peers(peers_bytes: &[u8]) -> Vec<String> {
    peers_bytes
        .chunks(6)
        .map(|chunk| {
            let ip = format!("{}.{}.{}.{}", chunk[0], chunk[1], chunk[2], chunk[3]);
            let port = u16::from_be_bytes([chunk[4], chunk[5]]);
            format!("{}:{}", ip, port)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_peer_id() {
        let peer_id = generate_peer_id();
        assert_eq!(peer_id.len(), 20);
        assert_eq!(&peer_id[..8], b"-RS0001-");
    }

    #[test]
    fn test_create_handshake() {
        let info_hash = [1u8; 20];
        let peer_id = [2u8; 20];
        let handshake = create_handshake(&info_hash, &peer_id);

        assert_eq!(handshake.len(), 68);
        assert_eq!(handshake[0], 19);
        assert_eq!(&handshake[1..20], b"BitTorrent protocol");
        assert_eq!(&handshake[28..48], &info_hash);
        assert_eq!(&handshake[48..68], &peer_id);
    }

    #[test]
    fn test_parse_peers() {
        let peers_bytes = vec![192, 168, 1, 1, 0x1A, 0xE1, 10, 0, 0, 1, 0x1A, 0xE2];
        let peers = parse_peers(&peers_bytes);

        assert_eq!(peers.len(), 2);
        assert_eq!(peers[0], "192.168.1.1:6881");
        assert_eq!(peers[1], "10.0.0.1:6882");
    }
}
