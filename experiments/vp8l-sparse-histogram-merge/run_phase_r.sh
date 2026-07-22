#!/usr/bin/env bash
set -euo pipefail

p21_root=$(cd "$(dirname "$0")/../.." && pwd)
p21_output=${1:?usage: run_phase_r.sh OUTPUT_DIR [CORPUS] [P18_WORKTREE]}
p21_corpus=${2:-/Users/lance/Desktop/webp/third_party/benchdata/clic/vp8l-lossless-exact}
p21_p18=${3:-/Users/lance/.codex/worktrees/7d78/webp}
p21_manifest_sha=9feb09f469753c43864011aa6f00cfc5ee1bd48da5aac8f9b16d105890e14f86
p21_screen_sha=474587feabe3178268b2eab6f7a166501d8ecc3d637a76bd412f4233dfa7b913
p21_p18_head=c04bed7bf044dc610081ff1de0e43a2a579258bb
p21_test=vp8l::image_writer::sparse_merge_benchmark_tests::sparse_merge_reproducer
p21_p18_test=vp8l::image_writer::product_benchmark_tests::product_validation_reproducer

test ! -e "$p21_output"
test "$(git -C "$p21_root" branch --show-current)" = codex/vp8l-sparse-histogram-merge
test "$(git -C "$p21_p18" rev-parse HEAD)" = "$p21_p18_head"
mkdir "$p21_output"
mkdir "$p21_output/raw"

find "$p21_corpus" -maxdepth 1 -type f -name '*-m6.webp' -print | sort |
while IFS= read -r p21_path; do
  p21_name=${p21_path##*/}
  p21_bytes=$(stat -f %z "$p21_path")
  p21_sha=$(shasum -a 256 "$p21_path" | cut -d ' ' -f 1)
  printf '%s\t%s\t%s\n' "$p21_name" "$p21_bytes" "$p21_sha"
done > "$p21_output/raw/corpus-manifest-102.tsv"
test "$(wc -l < "$p21_output/raw/corpus-manifest-102.tsv" | tr -d ' ')" -eq 102
test "$(shasum -a 256 "$p21_output/raw/corpus-manifest-102.tsv" | cut -d ' ' -f 1)" = "$p21_manifest_sha"
head -n 41 "$p21_output/raw/corpus-manifest-102.tsv" > "$p21_output/raw/screen-manifest-41.tsv"
test "$(shasum -a 256 "$p21_output/raw/screen-manifest-41.tsv" | cut -d ' ' -f 1)" = "$p21_screen_sha"

mkdir "$p21_output/raw/screen-input"
while IFS=$'\t' read -r p21_name _; do
  ln -s "$p21_corpus/$p21_name" "$p21_output/raw/screen-input/$p21_name"
done < "$p21_output/raw/screen-manifest-41.tsv"

CARGO_TARGET_DIR="$p21_output/raw/p21-target" cargo test \
  --manifest-path "$p21_root/webp-rs/Cargo.toml" -p webp --lib --release --no-run \
  > "$p21_output/raw/p21-build.log" 2>&1
p21_binary=$(find "$p21_output/raw/p21-target/release/deps" -maxdepth 1 -type f \
  -perm -111 -name 'webp-*' -print)
test "$(printf '%s\n' "$p21_binary" | sed '/^$/d' | wc -l | tr -d ' ')" -eq 1
p21_binary_sha=$(shasum -a 256 "$p21_binary" | cut -d ' ' -f 1)
"$p21_binary" --list "$p21_test" > "$p21_output/raw/final-binary-filter.txt"
grep -Fx "$p21_test: test" "$p21_output/raw/final-binary-filter.txt"

"$p21_binary" exact_block_frequencies::tests --nocapture \
  > "$p21_output/raw/mechanism-tests.log" 2>&1
env VP8L_SPARSE_COMMAND=phase-r VP8L_SPARSE_INPUT="$p21_corpus" \
  "$p21_binary" --exact "$p21_test" --ignored --nocapture \
  > "$p21_output/raw/phase-r-102.tsv" 2> "$p21_output/raw/phase-r-102.stderr"
env VP8L_SPARSE_COMMAND=census VP8L_SPARSE_INPUT="$p21_corpus" \
  "$p21_binary" --exact "$p21_test" --ignored --nocapture \
  > "$p21_output/raw/census-102.tsv" 2> "$p21_output/raw/census-102.stderr"

CARGO_TARGET_DIR="$p21_output/raw/p18-target" cargo test \
  --manifest-path "$p21_p18/webp-rs/Cargo.toml" -p webp --lib --release \
  --features vp8l-profile-hybrid-experiment --no-run \
  > "$p21_output/raw/p18-build.log" 2>&1
p21_p18_binary=$(find "$p21_output/raw/p18-target/release/deps" -maxdepth 1 -type f \
  -perm -111 -name 'webp-*' -print)
test "$(printf '%s\n' "$p21_p18_binary" | sed '/^$/d' | wc -l | tr -d ' ')" -eq 1
p21_p18_binary_sha=$(shasum -a 256 "$p21_p18_binary" | cut -d ' ' -f 1)
for p21_profile in compact low-latency; do
  env VP8L_PRODUCT_COMMAND=bench-encode VP8L_PRODUCT_INPUT="$p21_corpus" \
    VP8L_PRODUCT_LAYOUT="$p21_profile" VP8L_PRODUCT_ROUND=p18-oracle \
    "$p21_p18_binary" --exact "$p21_p18_test" --ignored --nocapture \
    > "$p21_output/raw/p18-$p21_profile.tsv" \
    2> "$p21_output/raw/p18-$p21_profile.stderr"
done

p21_measure() {
  p21_sequence=$1
  p21_round=$2
  p21_profile=$3
  p21_variant=$4
  p21_stem=$(printf '%02d-%s-%s-%s' "$p21_sequence" "$p21_round" "$p21_profile" "$p21_variant")
  env VP8L_SPARSE_COMMAND=bench VP8L_SPARSE_INPUT="$p21_output/raw/screen-input" \
    VP8L_SPARSE_PROFILE="$p21_profile" VP8L_SPARSE_VARIANT="$p21_variant" \
    VP8L_SPARSE_ROUND="$p21_round" "$p21_binary" --exact "$p21_test" \
    --ignored --nocapture > "$p21_output/raw/$p21_stem.tsv" \
    2> "$p21_output/raw/$p21_stem.stderr"
}

# One unscored warmup in forward order.
p21_measure 1 warmup compact a
p21_measure 2 warmup compact b
p21_measure 3 warmup low-latency a
p21_measure 4 warmup low-latency b
# Three retained full rounds in forward / reverse / forward order.
p21_measure 5 r1 compact a
p21_measure 6 r1 compact b
p21_measure 7 r1 low-latency a
p21_measure 8 r1 low-latency b
p21_measure 9 r2 low-latency b
p21_measure 10 r2 low-latency a
p21_measure 11 r2 compact b
p21_measure 12 r2 compact a
p21_measure 13 r3 compact a
p21_measure 14 r3 compact b
p21_measure 15 r3 low-latency a
p21_measure 16 r3 low-latency b

printf 'task=%s\nroot_task=%s\nbranch=%s\nbase=%s\nhead=%s\nworktree=%s\np21_binary=%s\np21_binary_sha256=%s\np18_binary=%s\np18_binary_sha256=%s\ncorpus_manifest_sha256=%s\nscreen_manifest_sha256=%s\n' \
  'P21 sparse exact-histogram merge recovery' \
  019f8321-035e-7211-8f53-987e18891c8c \
  codex/vp8l-sparse-histogram-merge \
  8485fc0593bf6e29715350ea72b15a9dabf4c80b \
  "$(git -C "$p21_root" rev-parse HEAD)" "$p21_root" "$p21_binary" "$p21_binary_sha" \
  "$p21_p18_binary" "$p21_p18_binary_sha" "$p21_manifest_sha" "$p21_screen_sha" \
  > "$p21_output/raw/binary-provenance.txt"

python3 "$p21_root/experiments/vp8l-sparse-histogram-merge/summarize_phase_r.py" \
  "$p21_output"
printf 'phase_r_complete=true\n' > "$p21_output/status.txt"
