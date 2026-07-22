#!/usr/bin/env bash
set -euo pipefail

p25_root=$(cd "$(dirname "$0")/../.." && pwd)
p25_output=${1:?usage: run_phase_r.sh OUTPUT_DIR [CORPUS] [P18_WORKTREE]}
p25_corpus=${2:-/Users/lance/Desktop/webp/third_party/benchdata/clic/vp8l-lossless-exact}
p25_p18=${3:-/Users/lance/.codex/worktrees/7d78/webp}
p25_test=vp8l::image_writer::product_benchmark_tests::product_validation_reproducer

test ! -e "$p25_output"
test "$(git -C "$p25_root" branch --show-current)" = codex/vp8l-entropy-map-frequency-sink
test -z "$(git -C "$p25_root" status --porcelain)"
test "$(git -C "$p25_p18" rev-parse HEAD)" = c04bed7bf044dc610081ff1de0e43a2a579258bb
test -z "$(git -C "$p25_p18" status --porcelain)"
mkdir -p "$p25_output/raw"

find "$p25_corpus" -maxdepth 1 -type f -name '*-m6.webp' -print | sort |
while IFS= read -r p25_path; do
  p25_name=${p25_path##*/}
  printf '%s\t%s\t%s\n' "$p25_name" "$(stat -f %z "$p25_path")" \
    "$(shasum -a 256 "$p25_path" | cut -d ' ' -f 1)"
done > "$p25_output/raw/corpus-manifest-102.tsv"
test "$(wc -l < "$p25_output/raw/corpus-manifest-102.tsv" | tr -d ' ')" -eq 102
test "$(shasum -a 256 "$p25_output/raw/corpus-manifest-102.tsv" | cut -d ' ' -f 1)" = \
  9feb09f469753c43864011aa6f00cfc5ee1bd48da5aac8f9b16d105890e14f86

CARGO_TARGET_DIR="$p25_output/raw/p25-target" cargo test \
  --manifest-path "$p25_root/webp-rs/Cargo.toml" -p webp --lib --release --no-run \
  > "$p25_output/raw/p25-build.log" 2>&1
p25_binary=$(find "$p25_output/raw/p25-target/release/deps" -maxdepth 1 -type f -perm -111 -name 'webp-*' -print)
test "$(printf '%s\n' "$p25_binary" | sed '/^$/d' | wc -l | tr -d ' ')" -eq 1

CARGO_TARGET_DIR="$p25_output/raw/p18-target" cargo test \
  --manifest-path "$p25_p18/webp-rs/Cargo.toml" -p webp --lib --release \
  --features vp8l-profile-hybrid-experiment --no-run > "$p25_output/raw/p18-build.log" 2>&1
p25_p18_binary=$(find "$p25_output/raw/p18-target/release/deps" -maxdepth 1 -type f -perm -111 -name 'webp-*' -print)
test "$(printf '%s\n' "$p25_p18_binary" | sed '/^$/d' | wc -l | tr -d ' ')" -eq 1

for p25_layout in compact low-latency compact-rank-sum low-latency-rank-sum compact-fused-rank-sum low-latency-fused-rank-sum; do
  env VP8L_PRODUCT_COMMAND=bench-encode VP8L_PRODUCT_INPUT="$p25_corpus" \
    VP8L_PRODUCT_LAYOUT="$p25_layout" VP8L_PRODUCT_ROUND=phase-r \
    "$p25_binary" --exact "$p25_test" --ignored --nocapture \
    > "$p25_output/raw/$p25_layout.tsv" 2> "$p25_output/raw/$p25_layout.stderr"
done
for p25_layout in compact low-latency; do
  env VP8L_PRODUCT_COMMAND=bench-encode VP8L_PRODUCT_INPUT="$p25_corpus" \
    VP8L_PRODUCT_LAYOUT="$p25_layout" VP8L_PRODUCT_ROUND=p18-oracle \
    "$p25_p18_binary" --exact "$p25_test" --ignored --nocapture \
    > "$p25_output/raw/p18-$p25_layout.tsv" 2> "$p25_output/raw/p18-$p25_layout.stderr"
done

env VP8L_PRODUCT_COMMAND=exact-cost-audit VP8L_PRODUCT_INPUT="$p25_corpus" \
  "$p25_binary" --exact "$p25_test" --ignored --nocapture \
  > "$p25_output/raw/candidate-census.tsv" 2> "$p25_output/raw/candidate-census.stderr"

printf 'p25_binary_sha256=%s\np18_binary_sha256=%s\n' \
  "$(shasum -a 256 "$p25_binary" | cut -d ' ' -f 1)" \
  "$(shasum -a 256 "$p25_p18_binary" | cut -d ' ' -f 1)" > "$p25_output/raw/binary-hashes.txt"
printf 'task=P25\nroot_task=019f8321-035e-7211-8f53-987e18891c8c\nbranch=%s\nbase=acfe6caf9fb62468dc384790b3e2eecfe837f173\nmeasurement_head=%s\nworktree=%s\np18_head=%s\n' \
  "$(git -C "$p25_root" branch --show-current)" "$(git -C "$p25_root" rev-parse HEAD)" \
  "$p25_root" "$(git -C "$p25_p18" rev-parse HEAD)" > "$p25_output/raw/provenance.txt"
python3 "$p25_root/experiments/vp8l-entropy-map-frequency-sink/summarize_phase_r.py" "$p25_output"
find "$p25_output" -type f ! -name SHA256SUMS -print | sort | while IFS= read -r p25_path; do
  shasum -a 256 "$p25_path"
done > "$p25_output/SHA256SUMS"
