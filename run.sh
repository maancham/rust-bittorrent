#!/bin/sh
#
# Use this script to run your program locally.
#

set -e

# Build once if not already built
if [ ! -f /tmp/codecrafters-build-bittorrent-rust/release/codecrafters-bittorrent ]; then
  cargo build --release --target-dir=/tmp/codecrafters-build-bittorrent-rust
fi

# Run the binary
exec /tmp/codecrafters-build-bittorrent-rust/release/codecrafters-bittorrent "$@"
