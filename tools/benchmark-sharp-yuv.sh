#!/usr/bin/env bash
# Measure direct Rust SharpYUV sampling on the pinned reference corpus.
set -euo pipefail

iterations="${1:-20}"
if ! [[ "$iterations" =~ ^[1-9][0-9]*$ ]]; then
  echo "usage: $0 [positive iterations]" >&2
  exit 2
fi

root="$(cd "$(dirname "$0")/.." && pwd)"
corpus="$root/third_party/corpus/reference-v1"

if [[ ! -d "$corpus/manifests" ]]; then
  echo "generate the pinned reference corpus before benchmarking:" >&2
  echo "  bash tools/generate-reference-corpus.sh" >&2
  exit 1
fi

inputs=()
for manifest in "$corpus"/manifests/*.toml; do
  if rg -q '^class = "MustAccept"$' "$manifest"; then
    file="$(sed -n 's|^file = "../\(.*\)"|\1|p' "$manifest")"
    inputs+=("$corpus/$file")
  fi
done
if [[ "${#inputs[@]}" -eq 0 ]]; then
  echo "no accepted SharpYUV benchmark inputs found" >&2
  exit 1
fi

rust_output="$(cargo run --quiet --release -p webp --example sharp_yuv_bench --features fuzzing \
  --manifest-path "$root/webp-rs/Cargo.toml" -- "$iterations" "${inputs[@]}")"
printf '%s\n' "$rust_output"

rust_checksum="$(printf '%s\n' "$rust_output" | sed -n 's/.* checksum=\([0-9][0-9]*\).*/\1/p')"
if [[ -z "$rust_checksum" ]]; then
  echo "SharpYUV benchmark did not emit a checksum" >&2
  exit 1
fi
