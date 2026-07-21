#!/usr/bin/env bash
# Benchmark the public static VP8L encoder against the pinned MustAccept corpus.
set -euo pipefail

iterations="${1:-5}"
if ! [[ "$iterations" =~ ^[1-9][0-9]*$ ]]; then
  echo "usage: $0 [positive iterations]" >&2
  exit 2
fi

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
corpus="$root/third_party/corpus/libwebp-test-data"
if [[ ! -d "$corpus/manifests" ]]; then
  echo "fetch the pinned corpus before benchmarking:" >&2
  echo "  tools/fetch-libwebp-test-data.sh" >&2
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

cargo build --release -p webp --example encode_bench --manifest-path "$root/Cargo.toml"
"$root/target/release/examples/encode_bench" "$iterations" "${inputs[@]}"
