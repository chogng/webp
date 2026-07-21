#!/usr/bin/env bash
# Benchmark the public lossy VP8 encoder over the pinned accepted corpus.
set -euo pipefail

iterations="${1:-5}"
if ! [[ "$iterations" =~ ^[1-9][0-9]*$ ]]; then
  echo "usage: $0 [positive iterations]" >&2
  exit 2
fi
root="$(cd "$(dirname "$0")/.." && pwd)"
corpus="$root/third_party/corpus/reference-v1"
oracle="$root/third_party/oracle/libwebp"
lockfile="$root/tools/corpus-lock.toml"

if [[ ! -d "$corpus/manifests" || ! -f "$oracle/build/libwebp.a" ]]; then
  echo "fetch the pinned corpus and oracle before benchmarking:" >&2
  echo "  bash tools/generate-reference-corpus.sh" >&2
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

scratch="$(mktemp -d "${TMPDIR:-/tmp}/webp-vp8-encode-bench.XXXXXX")"
trap 'rm -rf "$scratch"' EXIT
native="$scratch/libwebp_vp8_encode_bench"
cc -O3 -I"$oracle/src" "$root/tools/libwebp_vp8_encode_bench.c" \
  "$oracle/build/libwebp.a" "$oracle/build/libsharpyuv.a" -lm -o "$native"
"$native" "$iterations" "${files[@]}"
