mod bencode;
mod magnet;
mod peer;
mod torrent;
mod utils;

use magnet::MagnetLink;
use std::env;
use torrent::Torrent;

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let args: Vec<String> = env::args().collect();
    let command = &args[1];

    match command.as_str() {
        "decode" => {
            let encoded_value = &args[2];
            let decoded_value = bencode::decode(encoded_value);
            println!("{}", decoded_value.to_string());
        }
        "info" => {
            let torrent = Torrent::from_file(&args[2]);

            println!("Tracker URL: {}", torrent.announce);
            println!("Length: {}", torrent.length);
            println!("Info Hash: {}", torrent.info_hash_hex());
            println!("Piece Length: {}", torrent.piece_length);
            println!("Piece Hashes:");
            torrent
                .piece_hashes
                .iter()
                .for_each(|hash| println!("{}", hash));
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
        "download" => {
            let output_flag = &args[2];
            assert_eq!(output_flag, "-o", "Expected -o flag");
            let output_path = &args[3];
            let torrent_file = &args[4];

            let torrent = Torrent::from_file(torrent_file);
            torrent.download(output_path);
            println!("Downloaded {} to {}.", torrent_file, output_path);
        }
        "magnet_parse" => {
            let magnet_link = &args[2];
            let magnet = MagnetLink::parse(magnet_link);

            println!("Tracker URL: {}", magnet.tracker_url.as_deref().unwrap_or("N/A"));
            println!("Info Hash: {}", magnet.info_hash);
        }
        "magnet_info" => {
            let magnet_link = &args[2];
            let magnet = MagnetLink::parse(magnet_link);

            let peers = magnet.discover_peers();
            let peer_addr = &peers[0];

            magnet.info(peer_addr);
        }
        "magnet_handshake" => {
            let magnet_link = &args[2];
            let magnet = MagnetLink::parse(magnet_link);

            let peers = magnet.discover_peers();
            let peer_addr = &peers[0];

            let (peer_id, metadata_ext_id) = magnet.handshake(peer_addr);
            println!("Peer ID: {}", peer_id);
            if let Some(id) = metadata_ext_id {
                println!("Peer Metadata Extension ID: {}", id);
            }
        }
        _ => println!("unknown command: {}", command),
    }
}
