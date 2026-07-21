#!/usr/bin/env bash
# Benchmark the public bounded VP8L-frame animation encoder profile.
set -euo pipefail

iterations="${1:-5}"
if ! [[ "$iterations" =~ ^[1-9][0-9]*$ ]]; then
  echo "usage: $0 [positive iterations]" >&2
  exit 2
fi

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cargo build --release -p webp --example animation_encode_bench \
  --manifest-path "$root/webp-rs/Cargo.toml"
"$root/target/release/examples/animation_encode_bench" "$iterations"
