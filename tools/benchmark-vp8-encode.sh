#!/usr/bin/env bash
# Benchmark the public lossy VP8 encoder over the pinned accepted corpus.
set -euo pipefail

iterations="${1:-5}"
root="$(cd "$(dirname "$0")/.." && pwd)"
corpus="$root/third_party/corpus/reference-v1"

if [[ ! -d "$corpus/manifests" ]]; then
  echo "fetch the pinned corpus before benchmarking:" >&2
  echo "  bash tools/generate-reference-corpus.sh" >&2
  exit 1
fi

files=()
for manifest in "$corpus"/manifests/lossy-*.toml; do
  if rg -q '^class = "MustAccept"$' "$manifest"; then
    file="$(sed -n 's|^file = "../\(.*\)"|\1|p' "$manifest")"
    files+=("$corpus/$file")
  fi
done

if [[ "${#files[@]}" -eq 0 ]]; then
  echo "no accepted lossy VP8 benchmark inputs found" >&2
  exit 1
fi

cargo build --release -p webp --example vp8_encode_bench --manifest-path "$root/Cargo.toml"
"$root/target/release/examples/vp8_encode_bench" "$iterations" "${files[@]}"
