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
oracle="$root/third_party/oracle/libwebp"
lockfile="$root/tools/corpus-lock.toml"

if [[ ! -d "$corpus/manifests" || ! -f "$oracle/build/libsharpyuv.a" ]]; then
  echo "generate the pinned reference corpus and oracle before benchmarking:" >&2
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

scratch="$(mktemp -d "${TMPDIR:-/tmp}/webp-sharp-yuv-bench.XXXXXX")"
trap 'rm -rf "$scratch"' EXIT
rgba_corpus="$scratch/rgba-corpus.bin"
SHARP_YUV_RGBA_CORPUS="$rgba_corpus" \
  cargo run --quiet --release -p webp --example sharp_yuv_bench \
    --manifest-path "$root/webp-rs/Cargo.toml" -- "$iterations" "${inputs[@]}"

native="$scratch/libsharpyuv_bench"
cc -O3 -I"$oracle" -I"$oracle/src" "$root/tools/libsharpyuv_bench.c" \
  "$oracle/build/libsharpyuv.a" -lm -o "$native"
"$native" simd "$iterations" "$rgba_corpus"
"$native" scalar "$iterations" "$rgba_corpus"
