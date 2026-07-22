#!/usr/bin/env bash
# Compare Rust and pinned libwebp on the upstream transparent-image corpus.
set -euo pipefail

iterations="${1:-10}"
if ! [[ "$iterations" =~ ^[1-9][0-9]*$ ]]; then
  echo "usage: $0 [positive iterations]" >&2
  exit 2
fi

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
corpus="${WEBP_ALPHA_BENCH_CORPUS:-$root/third_party/corpus/libwebp-test-data}"
oracle="${WEBP_ALPHA_BENCH_LIBWEBP:-$root/third_party/oracle/libwebp}"
if [[ ! -d "$corpus/manifests" || ! -f "$oracle/build/libwebp.a" ]]; then
  echo "fetch the pinned corpus and oracle before benchmarking" >&2
  exit 1
fi

names=(
  alpha_color_cache.webp
  alpha_filter_0_method_0.webp
  alpha_filter_0_method_1.webp
  alpha_filter_1.webp
  alpha_filter_1_method_0.webp
  alpha_filter_1_method_1.webp
  alpha_filter_2.webp
  alpha_filter_2_method_0.webp
  alpha_filter_2_method_1.webp
  alpha_filter_3.webp
  alpha_filter_3_method_0.webp
  alpha_filter_3_method_1.webp
  alpha_no_compression.webp
  big_endian_bug_393.webp
  dual_transform.webp
  lossless1.webp
  lossless2.webp
  lossless3.webp
  lossless4.webp
  lossless_big_random_alpha.webp
  lossless_vec_1_0.webp
  lossless_vec_1_1.webp
  lossless_vec_1_10.webp
  lossless_vec_1_11.webp
  lossless_vec_1_12.webp
  lossless_vec_1_13.webp
  lossless_vec_1_14.webp
  lossless_vec_1_15.webp
  lossless_vec_1_2.webp
  lossless_vec_1_3.webp
  lossless_vec_1_4.webp
  lossless_vec_1_5.webp
  lossless_vec_1_6.webp
  lossless_vec_1_7.webp
  lossless_vec_1_8.webp
  lossless_vec_1_9.webp
  lossy_alpha1.webp
  lossy_alpha2.webp
  lossy_alpha3.webp
  lossy_alpha4.webp
  one_color_no_palette.webp
)
inputs=()
for name in "${names[@]}"; do
  inputs+=("$corpus/$name")
done

expected_commit="$(sed -n '/^\[libwebp\]$/,/^\[/ s/^commit = "\([^"]*\)"/\1/p' "$root/tools/corpus-lock.toml")"
actual_commit="$(git -C "$oracle" rev-parse HEAD)"
if [[ -z "$expected_commit" || "$actual_commit" != "$expected_commit" ]]; then
  echo "libwebp oracle pin mismatch: expected $expected_commit, found $actual_commit" >&2
  exit 1
fi

echo "benchmark=alpha-encode-v3 iterations=$iterations files=${#inputs[@]}"
echo "metadata repo_commit=$(git -C "$root" rev-parse HEAD) libwebp_commit=$actual_commit"
echo "metadata os=$(uname -srm | tr ' ' '_')"
echo "metadata rustc=$(rustc --version | tr ' ' '_')"
echo "metadata cc=$(cc --version | sed -n '1p' | tr ' ' '_')"

cargo run --release -p webp --example alpha_encode_bench \
  --manifest-path "$root/webp-rs/Cargo.toml" -- "$iterations" "${inputs[@]}"

scratch="$(mktemp -d "${TMPDIR:-/tmp}/webp-alpha-encode-bench.XXXXXX")"
trap 'rm -rf "$scratch"' EXIT
native="$scratch/libwebp_alpha_encode_bench"
cc -O3 -I"$oracle/src" "$root/tools/libwebp_alpha_encode_bench.c" \
  "$oracle/build/libwebp.a" "$oracle/build/libsharpyuv.a" -lm -o "$native"
"$native" "$iterations" "${inputs[@]}"
