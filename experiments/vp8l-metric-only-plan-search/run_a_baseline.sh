#!/usr/bin/env bash
set -euo pipefail

p22_root=$(cd "$(dirname "$0")/../.." && pwd)
p22_output=${1:?usage: run_a_baseline.sh OUTPUT_DIR [CORPUS] [P18_WORKTREE]}
p22_corpus=${2:-/Users/lance/Desktop/webp/third_party/benchdata/clic/vp8l-lossless-exact}
p22_p18=${3:-/Users/lance/.codex/worktrees/7d78/webp}
p22_branch=codex/vp8l-metric-only-plan-search
p22_base=4280a59a1a7a22d1e312b9de131b46873688c008
p22_p18_head=c04bed7bf044dc610081ff1de0e43a2a579258bb
p22_manifest_sha=9feb09f469753c43864011aa6f00cfc5ee1bd48da5aac8f9b16d105890e14f86
p22_screen_sha=474587feabe3178268b2eab6f7a166501d8ecc3d637a76bd412f4233dfa7b913
p22_test=vp8l::image_writer::product_benchmark_tests::product_validation_reproducer

test ! -e "$p22_output"
test "$(git -C "$p22_root" branch --show-current)" = "$p22_branch"
test "$(git -C "$p22_p18" rev-parse HEAD)" = "$p22_p18_head"
mkdir "$p22_output"
mkdir "$p22_output/raw"

find "$p22_corpus" -maxdepth 1 -type f -name '*-m6.webp' -print | sort |
while IFS= read -r p22_path; do
  p22_name=${p22_path##*/}
  p22_bytes=$(stat -f %z "$p22_path")
  p22_sha=$(shasum -a 256 "$p22_path" | cut -d ' ' -f 1)
  printf '%s\t%s\t%s\n' "$p22_name" "$p22_bytes" "$p22_sha"
done > "$p22_output/raw/corpus-manifest-102.tsv"
test "$(wc -l < "$p22_output/raw/corpus-manifest-102.tsv" | tr -d ' ')" -eq 102
test "$(shasum -a 256 "$p22_output/raw/corpus-manifest-102.tsv" | cut -d ' ' -f 1)" = \
  "$p22_manifest_sha"
head -n 41 "$p22_output/raw/corpus-manifest-102.tsv" > \
  "$p22_output/raw/screen-manifest-41.tsv"
test "$(shasum -a 256 "$p22_output/raw/screen-manifest-41.tsv" | cut -d ' ' -f 1)" = \
  "$p22_screen_sha"

CARGO_TARGET_DIR="$p22_output/raw/a-target" cargo test \
  --manifest-path "$p22_root/webp-rs/Cargo.toml" -p webp --lib --release --no-run \
  > "$p22_output/raw/a-build.log" 2>&1
p22_a_binary=$(find "$p22_output/raw/a-target/release/deps" -maxdepth 1 -type f \
  -perm -111 -name 'webp-*' -print)
test "$(printf '%s\n' "$p22_a_binary" | sed '/^$/d' | wc -l | tr -d ' ')" -eq 1
p22_a_binary_sha=$(shasum -a 256 "$p22_a_binary" | cut -d ' ' -f 1)
"$p22_a_binary" --list "$p22_test" > "$p22_output/raw/a-binary-filter.txt"
grep -Fx "$p22_test: test" "$p22_output/raw/a-binary-filter.txt"

CARGO_TARGET_DIR="$p22_output/raw/p18-target" cargo test \
  --manifest-path "$p22_p18/webp-rs/Cargo.toml" -p webp --lib --release \
  --features vp8l-profile-hybrid-experiment --no-run \
  > "$p22_output/raw/p18-build.log" 2>&1
p22_p18_binary=$(find "$p22_output/raw/p18-target/release/deps" -maxdepth 1 -type f \
  -perm -111 -name 'webp-*' -print)
test "$(printf '%s\n' "$p22_p18_binary" | sed '/^$/d' | wc -l | tr -d ' ')" -eq 1
p22_p18_binary_sha=$(shasum -a 256 "$p22_p18_binary" | cut -d ' ' -f 1)
"$p22_p18_binary" --list "$p22_test" > "$p22_output/raw/p18-binary-filter.txt"
grep -Fx "$p22_test: test" "$p22_output/raw/p18-binary-filter.txt"

for p22_profile in compact low-latency; do
  env VP8L_PRODUCT_COMMAND=bench-encode VP8L_PRODUCT_INPUT="$p22_corpus" \
    VP8L_PRODUCT_LAYOUT="$p22_profile" VP8L_PRODUCT_ROUND=a-baseline \
    "$p22_a_binary" --exact "$p22_test" --ignored --nocapture \
    > "$p22_output/raw/a-$p22_profile.tsv" \
    2> "$p22_output/raw/a-$p22_profile.stderr"
  env VP8L_PRODUCT_COMMAND=bench-encode VP8L_PRODUCT_INPUT="$p22_corpus" \
    VP8L_PRODUCT_LAYOUT="$p22_profile" VP8L_PRODUCT_ROUND=p18-oracle \
    "$p22_p18_binary" --exact "$p22_test" --ignored --nocapture \
    > "$p22_output/raw/p18-$p22_profile.tsv" \
    2> "$p22_output/raw/p18-$p22_profile.stderr"
done

printf 'task=%s\nroot_task=%s\nbranch=%s\nbase=%s\nhead=%s\nworktree=%s\na_binary=%s\na_binary_sha256=%s\np18_binary=%s\np18_binary_sha256=%s\ncorpus_manifest_sha256=%s\nscreen_manifest_sha256=%s\n' \
  'P22 independent VP8L metric-only search / final-plan materialization recovery experiment' \
  019f8321-035e-7211-8f53-987e18891c8c "$p22_branch" "$p22_base" \
  "$(git -C "$p22_root" rev-parse HEAD)" "$p22_root" "$p22_a_binary" "$p22_a_binary_sha" \
  "$p22_p18_binary" "$p22_p18_binary_sha" "$p22_manifest_sha" "$p22_screen_sha" \
  > "$p22_output/raw/binary-provenance.txt"

python3 "$p22_root/experiments/vp8l-metric-only-plan-search/summarize_a_baseline.py" \
  "$p22_output"
printf 'a_baseline_pass=true\n' > "$p22_output/status.txt"
