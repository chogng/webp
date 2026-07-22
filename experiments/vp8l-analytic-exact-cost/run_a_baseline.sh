#!/usr/bin/env bash
set -euo pipefail

p23_root=$(cd "$(dirname "$0")/../.." && pwd)
p23_output=${1:?usage: run_a_baseline.sh OUTPUT_DIR [CORPUS] [P18_WORKTREE]}
p23_corpus=${2:-/Users/lance/Desktop/webp/third_party/benchdata/clic/vp8l-lossless-exact}
p23_p18=${3:-/Users/lance/.codex/worktrees/7d78/webp}
p23_branch=codex/vp8l-analytic-exact-cost
p23_base=76c9aa39e35534b847f2cb980cb0037c4e6be785
p23_p18_head=c04bed7bf044dc610081ff1de0e43a2a579258bb
p23_manifest_sha=9feb09f469753c43864011aa6f00cfc5ee1bd48da5aac8f9b16d105890e14f86
p23_screen_sha=474587feabe3178268b2eab6f7a166501d8ecc3d637a76bd412f4233dfa7b913
p23_test=vp8l::image_writer::product_benchmark_tests::product_validation_reproducer

test ! -e "$p23_output"
test "$(git -C "$p23_root" branch --show-current)" = "$p23_branch"
test "$(git -C "$p23_p18" rev-parse HEAD)" = "$p23_p18_head"
mkdir "$p23_output"
mkdir "$p23_output/raw"

find "$p23_corpus" -maxdepth 1 -type f -name '*-m6.webp' -print | sort |
while IFS= read -r p23_path; do
  p23_name=${p23_path##*/}
  p23_bytes=$(stat -f %z "$p23_path")
  p23_sha=$(shasum -a 256 "$p23_path" | cut -d ' ' -f 1)
  printf '%s\t%s\t%s\n' "$p23_name" "$p23_bytes" "$p23_sha"
done > "$p23_output/raw/corpus-manifest-102.tsv"
test "$(wc -l < "$p23_output/raw/corpus-manifest-102.tsv" | tr -d ' ')" -eq 102
test "$(shasum -a 256 "$p23_output/raw/corpus-manifest-102.tsv" | cut -d ' ' -f 1)" = \
  "$p23_manifest_sha"
head -n 41 "$p23_output/raw/corpus-manifest-102.tsv" > \
  "$p23_output/raw/screen-manifest-41.tsv"
test "$(shasum -a 256 "$p23_output/raw/screen-manifest-41.tsv" | cut -d ' ' -f 1)" = \
  "$p23_screen_sha"

CARGO_TARGET_DIR="$p23_output/raw/a-target" cargo test \
  --manifest-path "$p23_root/webp-rs/Cargo.toml" -p webp --lib --release --no-run \
  > "$p23_output/raw/a-build.log" 2>&1
p23_a_binary=$(find "$p23_output/raw/a-target/release/deps" -maxdepth 1 -type f \
  -perm -111 -name 'webp-*' -print)
test "$(printf '%s\n' "$p23_a_binary" | sed '/^$/d' | wc -l | tr -d ' ')" -eq 1
p23_a_binary_sha=$(shasum -a 256 "$p23_a_binary" | cut -d ' ' -f 1)
"$p23_a_binary" --list "$p23_test" > "$p23_output/raw/a-binary-filter.txt"
grep -Fx "$p23_test: test" "$p23_output/raw/a-binary-filter.txt"

CARGO_TARGET_DIR="$p23_output/raw/p18-target" cargo test \
  --manifest-path "$p23_p18/webp-rs/Cargo.toml" -p webp --lib --release \
  --features vp8l-profile-hybrid-experiment --no-run \
  > "$p23_output/raw/p18-build.log" 2>&1
p23_p18_binary=$(find "$p23_output/raw/p18-target/release/deps" -maxdepth 1 -type f \
  -perm -111 -name 'webp-*' -print)
test "$(printf '%s\n' "$p23_p18_binary" | sed '/^$/d' | wc -l | tr -d ' ')" -eq 1
p23_p18_binary_sha=$(shasum -a 256 "$p23_p18_binary" | cut -d ' ' -f 1)
"$p23_p18_binary" --list "$p23_test" > "$p23_output/raw/p18-binary-filter.txt"
grep -Fx "$p23_test: test" "$p23_output/raw/p18-binary-filter.txt"

for p23_profile in compact low-latency; do
  env VP8L_PRODUCT_COMMAND=bench-encode VP8L_PRODUCT_INPUT="$p23_corpus" \
    VP8L_PRODUCT_LAYOUT="$p23_profile" VP8L_PRODUCT_ROUND=a-baseline \
    "$p23_a_binary" --exact "$p23_test" --ignored --nocapture \
    > "$p23_output/raw/a-$p23_profile.tsv" \
    2> "$p23_output/raw/a-$p23_profile.stderr"
  env VP8L_PRODUCT_COMMAND=bench-encode VP8L_PRODUCT_INPUT="$p23_corpus" \
    VP8L_PRODUCT_LAYOUT="$p23_profile" VP8L_PRODUCT_ROUND=p18-oracle \
    "$p23_p18_binary" --exact "$p23_test" --ignored --nocapture \
    > "$p23_output/raw/p18-$p23_profile.tsv" \
    2> "$p23_output/raw/p18-$p23_profile.stderr"
done

printf 'task=%s\nroot_task=%s\nbranch=%s\nbase=%s\nhead=%s\nworktree=%s\na_binary=%s\na_binary_sha256=%s\np18_binary=%s\np18_binary_sha256=%s\ncorpus_manifest_sha256=%s\nscreen_manifest_sha256=%s\n' \
  'P23 independent VP8L analytic exact-cost / selected-only materialization experiment' \
  019f8321-035e-7211-8f53-987e18891c8c "$p23_branch" "$p23_base" \
  "$(git -C "$p23_root" rev-parse HEAD)" "$p23_root" "$p23_a_binary" "$p23_a_binary_sha" \
  "$p23_p18_binary" "$p23_p18_binary_sha" "$p23_manifest_sha" "$p23_screen_sha" \
  > "$p23_output/raw/binary-provenance.txt"

python3 "$p23_root/experiments/vp8l-analytic-exact-cost/summarize_a_baseline.py" \
  "$p23_output"
printf 'a_baseline_pass=true\n' > "$p23_output/status.txt"
