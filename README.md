# BitTorrent Client in Rust

A BitTorrent client built from scratch in Rust. Core BitTorrent protocol includes bencode parsing, peer discovery, piece downloading, and the BitTorrent Extension Protocol (BEP 10) for magnet link support.

## Features

- **Bencode parser** — decodes integers, strings, lists, and dictionaries
- **Torrent file parsing** — reads `.torrent` files and extracts metadata (tracker URL, info hash, piece hashes)
- **Peer discovery** — queries HTTP trackers to find peers
- **Peer handshake** — implements the BitTorrent handshake protocol
- **Piece downloading** — downloads individual pieces or entire files using 16 KiB block requests, with SHA-1 verification on each piece
- **Magnet link support** — parses magnet links and fetches torrent metadata from peers at runtime using the `ut_metadata` extension (BEP 9 / BEP 10)

## Implementation Details

### Protocol layers implemented

| Layer | Details |
|---|---|
| Bencode | Full decoder for all four types |
| Tracker HTTP | `GET` request with compact peer response parsing |
| Base handshake | 68-byte BitTorrent handshake with extension bit negotiation |
| Wire messages | Length-prefixed framing: bitfield, interested, unchoke, request, piece |
| Extension protocol (BEP 10) | Extension handshake (`ut_metadata` advertisement and negotiation) |
| Metadata extension (BEP 9) | `request` and `data` message types; fetches info dictionary from peers |


## Usage

```bash
./run.sh <command> [args]
```

The script will build the project on first run, then use the cached binary for subsequent runs.

### Commands

**Decode a bencoded value:**
```bash
./run.sh decode "d3:foo3:bar5:helloi52ee"
```

**Inspect a `.torrent` file:**
```bash
./run.sh info sample.torrent
# Tracker URL: http://...
# Length: 92063
# Info Hash: d69f91e6b2ae4c542468d1073a71d4ea13879a7f
# Piece Length: 32768
# Piece Hashes:
# 6e2275e604a0766656736e81ff10b55204ad8d35
# ...
```

**Discover peers for a torrent:**
```bash
./run.sh peers sample.torrent
```

**Perform a handshake with a specific peer:**
```bash
./run.sh handshake sample.torrent 127.0.0.1:6881
```

**Download a single piece:**
```bash
./run.sh download_piece -o /tmp/piece-0 sample.torrent 0
```

**Download an entire file:**
```bash
./run.sh download -o /tmp/output sample.torrent
```

**Parse a magnet link:**
```bash
./run.sh magnet_parse "magnet:?xt=urn:btih:...&tr="
```

**Perform a handshake via magnet link (with extension negotiation):**
```bash
./run.sh magnet_handshake "magnet:?xt=urn:btih:...&tr="
# Peer ID: 0102030405060708090a0b0c0d0e0f1011121314
# Peer Metadata Extension ID: 3
```

**Fetch torrent info from a magnet link:**
```bash
./run.sh magnet_info "magnet:?xt=urn:btih:...&tr="
# Tracker URL: http://...
# Length: 92063
# Info Hash: d69f91e6b2ae4c542468d1073a71d4ea13879a7f
# Piece Length: 32768
# Piece Hashes:
# ...
```

**Download a single piece via magnet link:**
```bash
./run.sh magnet_download_piece -o /tmp/piece-0 "magnet:?xt=urn:btih:...&tr=" 0
```

**Download an entire file via magnet link:**
```bash
./run.sh magnet_download -o /tmp/output "magnet:?xt=urn:btih:...&tr="
```

## How Magnet Links Work

Magnet links don't bundle a `.torrent` file — they only carry an info hash and a tracker URL. This client resolves the gap at runtime:

1. Queries the tracker to get a list of peers
2. Performs a base handshake, advertising extension protocol support (BEP 10)
3. Exchanges extension handshakes to negotiate the `ut_metadata` extension ID
4. Requests the info dictionary from the peer using BEP 9 metadata messages
5. Parses the received info dictionary (identical to the `info` block in a `.torrent` file)
6. Proceeds with normal piece downloading on the same TCP connection

## Running Tests

```bash
cargo test
```
