#!/bin/sh
set -eu

p20_root=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
p20_output=${1:?usage: run_phase_a.sh OUTPUT_DIR [CORPUS] [P18_WORKTREE]}
p20_corpus=${2:-/Users/lance/Desktop/webp/third_party/benchdata/clic/vp8l-lossless-exact}
p20_p18=${3:-/Users/lance/.codex/worktrees/7d78/webp}
p20_test=vp8l::image_writer::product_benchmark_tests::product_validation_reproducer
p20_manifest_sha=9feb09f469753c43864011aa6f00cfc5ee1bd48da5aac8f9b16d105890e14f86

mkdir "$p20_output"
mkdir "$p20_output/raw"
find "$p20_corpus" -maxdepth 1 -type f -name '*-m6.webp' -print | sort |
while IFS= read -r p20_path; do
  p20_name=${p20_path##*/}
  p20_bytes=$(stat -f %z "$p20_path")
  p20_sha=$(shasum -a 256 "$p20_path" | cut -d ' ' -f 1)
  printf '%s\t%s\t%s\n' "$p20_name" "$p20_bytes" "$p20_sha"
done > "$p20_output/raw/corpus-manifest-102.tsv"
test "$(wc -l < "$p20_output/raw/corpus-manifest-102.tsv" | tr -d ' ')" -eq 102
test "$(shasum -a 256 "$p20_output/raw/corpus-manifest-102.tsv" | cut -d ' ' -f 1)" = "$p20_manifest_sha"
head -n 41 "$p20_output/raw/corpus-manifest-102.tsv" > "$p20_output/raw/screen-manifest-41.tsv"

P20_PRODUCT_TARGET="$p20_output/product-target"
export P20_PRODUCT_TARGET
CARGO_TARGET_DIR="$P20_PRODUCT_TARGET" cargo test --manifest-path "$p20_root/webp-rs/Cargo.toml" \
  -p webp --release --no-run > "$p20_output/raw/product-build.log" 2>&1
p20_binary=$(find "$P20_PRODUCT_TARGET/release/deps" -maxdepth 1 -type f -perm -111 -name 'webp-*' -print)
test "$(printf '%s\n' "$p20_binary" | sed '/^$/d' | wc -l | tr -d ' ')" -eq 1

P20_P18_TARGET="$p20_output/p18-target"
export P20_P18_TARGET
CARGO_TARGET_DIR="$P20_P18_TARGET" cargo test --manifest-path "$p20_p18/webp-rs/Cargo.toml" \
  -p webp --release --features vp8l-profile-hybrid-experiment --no-run \
  > "$p20_output/raw/p18-build.log" 2>&1
p20_p18_binary=$(find "$P20_P18_TARGET/release/deps" -maxdepth 1 -type f -perm -111 -name 'webp-*' -print)
test "$(printf '%s\n' "$p20_p18_binary" | sed '/^$/d' | wc -l | tr -d ' ')" -eq 1

env VP8L_PRODUCT_COMMAND=product-phase-a VP8L_PRODUCT_INPUT="$p20_corpus" \
  "$p20_binary" --exact "$p20_test" --ignored --nocapture \
  > "$p20_output/raw/product-phase-a.tsv" 2> "$p20_output/raw/product-phase-a.stderr"
for p20_profile in compact low-latency; do
  env VP8L_PRODUCT_COMMAND=bench-encode VP8L_PRODUCT_INPUT="$p20_corpus" \
    VP8L_PRODUCT_LAYOUT="$p20_profile" VP8L_PRODUCT_ROUND=p18-oracle \
    "$p20_p18_binary" --exact "$p20_test" --ignored --nocapture \
    > "$p20_output/raw/p18-$p20_profile.tsv" \
    2> "$p20_output/raw/p18-$p20_profile.stderr"
done

python3 "$p20_root/experiments/vp8l-profile-hybrid-product/summarize_phase_a.py" \
  --product "$p20_output/raw/product-phase-a.tsv" \
  --p18-compact "$p20_output/raw/p18-compact.tsv" \
  --p18-low-latency "$p20_output/raw/p18-low-latency.tsv" \
  --output "$p20_output/phase-a-summary.json"
printf 'product_binary=%s\nproduct_binary_sha256=%s\np18_binary=%s\np18_binary_sha256=%s\n' \
  "$p20_binary" "$(shasum -a 256 "$p20_binary" | cut -d ' ' -f 1)" \
  "$p20_p18_binary" "$(shasum -a 256 "$p20_p18_binary" | cut -d ' ' -f 1)" \
  > "$p20_output/binary-provenance.txt"
printf 'phase_a_pass=true\n' > "$p20_output/status.txt"
