#!/usr/bin/env bash
set -euo pipefail

p24_root=$(cd "$(dirname "$0")/../.." && pwd)
p24_output=${1:?usage: run_a_baseline.sh OUTPUT_DIR [CORPUS] [P18_WORKTREE]}
p24_corpus=${2:-/Users/lance/Desktop/webp/third_party/benchdata/clic/vp8l-lossless-exact}
p24_p18=${3:-/Users/lance/.codex/worktrees/7d78/webp}
p24_branch=codex/vp8l-rank-sum-exact-cost
p24_base=230ce0bd1c201d2687261d97a525cec8f91aa215
p24_p18_head=c04bed7bf044dc610081ff1de0e43a2a579258bb
p24_manifest_sha=9feb09f469753c43864011aa6f00cfc5ee1bd48da5aac8f9b16d105890e14f86
p24_screen_sha=474587feabe3178268b2eab6f7a166501d8ecc3d637a76bd412f4233dfa7b913
p24_test=vp8l::image_writer::product_benchmark_tests::product_validation_reproducer

test ! -e "$p24_output"
test "$(git -C "$p24_root" branch --show-current)" = "$p24_branch"
test -z "$(git -C "$p24_root" status --porcelain)"
test "$(git -C "$p24_p18" rev-parse HEAD)" = "$p24_p18_head"
mkdir "$p24_output"
mkdir "$p24_output/raw"

find "$p24_corpus" -maxdepth 1 -type f -name '*-m6.webp' -print | sort |
while IFS= read -r p24_path; do
  p24_name=${p24_path##*/}
  p24_bytes=$(stat -f %z "$p24_path")
  p24_sha=$(shasum -a 256 "$p24_path" | cut -d ' ' -f 1)
  printf '%s\t%s\t%s\n' "$p24_name" "$p24_bytes" "$p24_sha"
done > "$p24_output/raw/corpus-manifest-102.tsv"
test "$(wc -l < "$p24_output/raw/corpus-manifest-102.tsv" | tr -d ' ')" -eq 102
test "$(shasum -a 256 "$p24_output/raw/corpus-manifest-102.tsv" | cut -d ' ' -f 1)" = \
  "$p24_manifest_sha"
head -n 41 "$p24_output/raw/corpus-manifest-102.tsv" > \
  "$p24_output/raw/screen-manifest-41.tsv"
test "$(shasum -a 256 "$p24_output/raw/screen-manifest-41.tsv" | cut -d ' ' -f 1)" = \
  "$p24_screen_sha"

CARGO_TARGET_DIR="$p24_output/raw/a-target" cargo test \
  --manifest-path "$p24_root/webp-rs/Cargo.toml" -p webp --lib --release --no-run \
  > "$p24_output/raw/a-build.log" 2>&1
p24_a_binary=$(find "$p24_output/raw/a-target/release/deps" -maxdepth 1 -type f \
  -perm -111 -name 'webp-*' -print)
test "$(printf '%s\n' "$p24_a_binary" | sed '/^$/d' | wc -l | tr -d ' ')" -eq 1
p24_a_binary_sha=$(shasum -a 256 "$p24_a_binary" | cut -d ' ' -f 1)
"$p24_a_binary" --list "$p24_test" > "$p24_output/raw/a-binary-filter.txt"
grep -Fx "$p24_test: test" "$p24_output/raw/a-binary-filter.txt"

CARGO_TARGET_DIR="$p24_output/raw/p18-target" cargo test \
  --manifest-path "$p24_p18/webp-rs/Cargo.toml" -p webp --lib --release \
  --features vp8l-profile-hybrid-experiment --no-run \
  > "$p24_output/raw/p18-build.log" 2>&1
p24_p18_binary=$(find "$p24_output/raw/p18-target/release/deps" -maxdepth 1 -type f \
  -perm -111 -name 'webp-*' -print)
test "$(printf '%s\n' "$p24_p18_binary" | sed '/^$/d' | wc -l | tr -d ' ')" -eq 1
p24_p18_binary_sha=$(shasum -a 256 "$p24_p18_binary" | cut -d ' ' -f 1)
"$p24_p18_binary" --list "$p24_test" > "$p24_output/raw/p18-binary-filter.txt"
grep -Fx "$p24_test: test" "$p24_output/raw/p18-binary-filter.txt"

for p24_profile in compact low-latency; do
  env VP8L_PRODUCT_COMMAND=bench-encode VP8L_PRODUCT_INPUT="$p24_corpus" \
    VP8L_PRODUCT_LAYOUT="$p24_profile" VP8L_PRODUCT_ROUND=a-baseline \
    "$p24_a_binary" --exact "$p24_test" --ignored --nocapture \
    > "$p24_output/raw/a-$p24_profile.tsv" \
    2> "$p24_output/raw/a-$p24_profile.stderr"
  env VP8L_PRODUCT_COMMAND=bench-encode VP8L_PRODUCT_INPUT="$p24_corpus" \
    VP8L_PRODUCT_LAYOUT="$p24_profile" VP8L_PRODUCT_ROUND=p18-oracle \
    "$p24_p18_binary" --exact "$p24_test" --ignored --nocapture \
    > "$p24_output/raw/p18-$p24_profile.tsv" \
    2> "$p24_output/raw/p18-$p24_profile.stderr"
done

printf 'task=%s\nroot_task=%s\nbranch=%s\nbase=%s\nhead=%s\nworktree=%s\na_binary=%s\na_binary_sha256=%s\np18_binary=%s\np18_binary_sha256=%s\ncorpus_manifest_sha256=%s\nscreen_manifest_sha256=%s\n' \
  'P24 independent VP8L allocation-free rank-sum exact-cost experiment' \
  019f8321-035e-7211-8f53-987e18891c8c "$p24_branch" "$p24_base" \
  "$(git -C "$p24_root" rev-parse HEAD)" "$p24_root" "$p24_a_binary" "$p24_a_binary_sha" \
  "$p24_p18_binary" "$p24_p18_binary_sha" "$p24_manifest_sha" "$p24_screen_sha" \
  > "$p24_output/raw/binary-provenance.txt"

python3 "$p24_root/experiments/vp8l-rank-sum-exact-cost/summarize_a_baseline.py" \
  "$p24_output"
printf 'a_baseline_pass=true\n' > "$p24_output/status.txt"

find "$p24_output" -type f ! -name SHA256SUMS -print | sort | while IFS= read -r p24_path; do
  shasum -a 256 "$p24_path"
done > "$p24_output/SHA256SUMS"
printf 'external_sha256sums_sha256=%s\n' \
  "$(shasum -a 256 "$p24_output/SHA256SUMS" | cut -d ' ' -f 1)" >> "$p24_output/status.txt"
