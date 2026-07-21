#!/usr/bin/env bash
# Compare Rust and pinned libwebp on the upstream transparent-image corpus.
set -euo pipefail

iterations="${1:-10}"
if ! [[ "$iterations" =~ ^[1-9][0-9]*$ ]]; then
  echo "usage: $0 [positive iterations]" >&2
  exit 2
fi

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
corpus="$root/third_party/corpus/libwebp-test-data"
oracle="$root/third_party/oracle/libwebp"
if [[ ! -d "$corpus/manifests" || ! -f "$oracle/build/libwebp.a" ]]; then
  echo "fetch the pinned corpus and oracle before benchmarking" >&2
  exit 1
fi

names=(
  alpha_no_compression.webp
  alpha_filter_0_method_0.webp
  alpha_filter_1_method_0.webp
  alpha_filter_2_method_0.webp
  alpha_filter_3_method_0.webp
  alpha_filter_0_method_1.webp
  alpha_filter_1_method_1.webp
  alpha_filter_2_method_1.webp
  alpha_filter_3_method_1.webp
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

cargo build --release -p webp --example alpha_encode_bench --manifest-path "$root/webp-rs/Cargo.toml"
"$root/webp-rs/target/release/examples/alpha_encode_bench" "$iterations" "${inputs[@]}"

scratch="$(mktemp -d "${TMPDIR:-/tmp}/webp-alpha-encode-bench.XXXXXX")"
trap 'rm -rf "$scratch"' EXIT
native="$scratch/libwebp_alpha_encode_bench"
cc -O3 -I"$oracle/src" "$root/tools/libwebp_alpha_encode_bench.c" \
  "$oracle/build/libwebp.a" "$oracle/build/libsharpyuv.a" -lm -o "$native"
"$native" "$iterations" "${inputs[@]}"
