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
oracle="$root/third_party/oracle/libwebp"
lockfile="$root/tools/corpus-lock.toml"
if [[ ! -d "$corpus/manifests" || ! -f "$oracle/build/libwebp.a" ]]; then
  echo "fetch the pinned corpus and oracle before benchmarking:" >&2
  echo "  tools/fetch-libwebp-test-data.sh" >&2
  echo "  tools/fetch-libwebp-oracle.sh" >&2
  exit 1
fi

expected_commit="$(awk -F ' = ' '
  $0 == "[libwebp]" { in_section = 1; next }
  /^\[/ { in_section = 0 }
  in_section && $1 == "commit" {
    value = $2
    gsub(/^"|"$/, "", value)
    print value
    exit
  }
' "$lockfile")"
actual_commit="$(git -C "$oracle" rev-parse HEAD)"
if [[ -z "$expected_commit" || "$actual_commit" != "$expected_commit" ]]; then
  echo "libwebp oracle pin mismatch: expected $expected_commit, found $actual_commit" >&2
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

cargo run --release -p webp --example encode_bench \
  --manifest-path "$root/webp-rs/Cargo.toml" -- "$iterations" "${inputs[@]}"

scratch="$(mktemp -d "${TMPDIR:-/tmp}/webp-vp8l-encode-bench.XXXXXX")"
trap 'rm -rf "$scratch"' EXIT
native="$scratch/libwebp_vp8l_encode_bench"
cc -O3 -I"$oracle/src" "$root/tools/libwebp_vp8l_encode_bench.c" \
  "$oracle/build/libwebp.a" "$oracle/build/libsharpyuv.a" -lm -o "$native"
"$native" "$iterations" "${inputs[@]}"
