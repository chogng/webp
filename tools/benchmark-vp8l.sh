#!/usr/bin/env bash
# Compare the public Rust VP8L decoder with the pinned libwebp C API.
set -euo pipefail

iterations="${1:-5}"
if ! [[ "$iterations" =~ ^[1-9][0-9]*$ ]]; then
  echo "usage: $0 [positive iterations]" >&2
  exit 2
fi

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
. "$root/tools/temporary.sh"
corpus="$root/third_party/corpus/libwebp-test-data"
oracle="$root/third_party/oracle/libwebp"
if [[ ! -d "$corpus/manifests" || ! -f "$oracle/build/libwebp.a" ]]; then
  echo "fetch the pinned corpus and oracle before benchmarking:" >&2
  echo "  tools/fetch-libwebp-test-data.sh" >&2
  echo "  tools/fetch-libwebp-oracle.sh" >&2
  exit 1
fi

inputs=()
for manifest in "$corpus"/manifests/*.toml; do
  if rg -q '^class = "MustAccept"$' "$manifest" && \
      rg -q '^codec = "VP8L"$' "$manifest" && \
      rg -q '^api = "Decode"$' "$manifest"; then
    file="$(sed -n 's|^file = "../\(.*\)"|\1|p' "$manifest")"
    inputs+=("$corpus/$file")
  fi
done
if [[ "${#inputs[@]}" -eq 0 ]]; then
  echo "no accepted VP8L benchmark inputs found" >&2
  exit 1
fi

scratch="$(webp_mktemp_dir "$root" webp-vp8l-bench)"
webp_cleanup_on_exit "$scratch"
native="$scratch/libwebp_decode_bench"
cc -O3 -I"$oracle/src" "$root/tools/libwebp_decode_bench.c" \
  "$oracle/build/libwebp.a" -o "$native"
cargo run --release -p webp --example decode_bench \
  --manifest-path "$root/webp-rs/Cargo.toml" -- "$iterations" "${inputs[@]}"
"$native" "$iterations" "${inputs[@]}"
