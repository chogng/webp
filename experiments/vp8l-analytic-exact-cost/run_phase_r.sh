#!/usr/bin/env bash
set -euo pipefail

p23_root=$(cd "$(dirname "$0")/../.." && pwd)
p23_output=${1:?usage: run_phase_r.sh OUTPUT_DIR [CORPUS] [P18_WORKTREE]}
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
test -z "$(git -C "$p23_root" status --porcelain)"
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
mkdir "$p23_output/raw/screen-input"
while IFS=$'\t' read -r p23_name _; do
  ln -s "$p23_corpus/$p23_name" "$p23_output/raw/screen-input/$p23_name"
done < "$p23_output/raw/screen-manifest-41.tsv"

CARGO_TARGET_DIR="$p23_output/raw/p23-target" cargo test \
  --manifest-path "$p23_root/webp-rs/Cargo.toml" -p webp --lib --release --no-run \
  > "$p23_output/raw/p23-build.log" 2>&1
p23_binary=$(find "$p23_output/raw/p23-target/release/deps" -maxdepth 1 -type f \
  -perm -111 -name 'webp-*' -print)
test "$(printf '%s\n' "$p23_binary" | sed '/^$/d' | wc -l | tr -d ' ')" -eq 1
p23_binary_sha=$(shasum -a 256 "$p23_binary" | cut -d ' ' -f 1)
"$p23_binary" --list "$p23_test" > "$p23_output/raw/final-binary-filter.txt"
grep -Fx "$p23_test: test" "$p23_output/raw/final-binary-filter.txt"
"$p23_binary" vp8l::image_writer --nocapture \
  > "$p23_output/raw/mechanism-tests.log" 2>&1

env VP8L_PRODUCT_COMMAND=analytic-audit VP8L_PRODUCT_INPUT="$p23_corpus" \
  "$p23_binary" --exact "$p23_test" --ignored --nocapture \
  > "$p23_output/raw/candidate-audit-102.tsv" \
  2> "$p23_output/raw/candidate-audit-102.stderr"

CARGO_TARGET_DIR="$p23_output/raw/p18-target" cargo test \
  --manifest-path "$p23_p18/webp-rs/Cargo.toml" -p webp --lib --release \
  --features vp8l-profile-hybrid-experiment --no-run \
  > "$p23_output/raw/p18-build.log" 2>&1
p23_p18_binary=$(find "$p23_output/raw/p18-target/release/deps" -maxdepth 1 -type f \
  -perm -111 -name 'webp-*' -print)
test "$(printf '%s\n' "$p23_p18_binary" | sed '/^$/d' | wc -l | tr -d ' ')" -eq 1
p23_p18_binary_sha=$(shasum -a 256 "$p23_p18_binary" | cut -d ' ' -f 1)

for p23_profile in compact low-latency; do
  if test "$p23_profile" = compact; then
    p23_b_layout=compact-analytic
  else
    p23_b_layout=low-latency-analytic
  fi
  for p23_variant in a public; do
    env VP8L_PRODUCT_COMMAND=bench-encode VP8L_PRODUCT_INPUT="$p23_corpus" \
      VP8L_PRODUCT_LAYOUT="$p23_profile" VP8L_PRODUCT_ROUND="$p23_variant-identity" \
      "$p23_binary" --exact "$p23_test" --ignored --nocapture \
      > "$p23_output/raw/$p23_variant-$p23_profile.tsv" \
      2> "$p23_output/raw/$p23_variant-$p23_profile.stderr"
  done
  env VP8L_PRODUCT_COMMAND=bench-encode VP8L_PRODUCT_INPUT="$p23_corpus" \
    VP8L_PRODUCT_LAYOUT="$p23_b_layout" VP8L_PRODUCT_ROUND=b-identity \
    "$p23_binary" --exact "$p23_test" --ignored --nocapture \
    > "$p23_output/raw/b-$p23_profile.tsv" \
    2> "$p23_output/raw/b-$p23_profile.stderr"
  env VP8L_PRODUCT_COMMAND=bench-encode VP8L_PRODUCT_INPUT="$p23_corpus" \
    VP8L_PRODUCT_LAYOUT="$p23_profile" VP8L_PRODUCT_ROUND=p18-oracle \
    "$p23_p18_binary" --exact "$p23_test" --ignored --nocapture \
    > "$p23_output/raw/p18-$p23_profile.tsv" \
    2> "$p23_output/raw/p18-$p23_profile.stderr"
done

p23_measure() {
  p23_sequence=$1
  p23_round=$2
  p23_profile=$3
  p23_variant=$4
  p23_layout=$p23_profile
  if test "$p23_variant" = b; then
    p23_layout="$p23_profile-analytic"
  fi
  p23_stem=$(printf '%02d-%s-%s-%s' \
    "$p23_sequence" "$p23_round" "$p23_profile" "$p23_variant")
  env VP8L_PRODUCT_COMMAND=bench-encode \
    VP8L_PRODUCT_INPUT="$p23_output/raw/screen-input" \
    VP8L_PRODUCT_LAYOUT="$p23_layout" VP8L_PRODUCT_ROUND="$p23_round" \
    "$p23_binary" --exact "$p23_test" --ignored --nocapture \
    > "$p23_output/raw/$p23_stem.tsv" \
    2> "$p23_output/raw/$p23_stem.stderr"
}

# Exactly one unscored warmup in forward profile/variant order.
p23_measure 1 warmup compact a
p23_measure 2 warmup compact b
p23_measure 3 warmup low-latency a
p23_measure 4 warmup low-latency b
# The only valid recovery sample: three retained F/R/F rounds.
p23_measure 5 r1 compact a
p23_measure 6 r1 compact b
p23_measure 7 r1 low-latency a
p23_measure 8 r1 low-latency b
p23_measure 9 r2 low-latency b
p23_measure 10 r2 low-latency a
p23_measure 11 r2 compact b
p23_measure 12 r2 compact a
p23_measure 13 r3 compact a
p23_measure 14 r3 compact b
p23_measure 15 r3 low-latency a
p23_measure 16 r3 low-latency b

printf 'task=%s\nroot_task=%s\nbranch=%s\nbase=%s\nhead=%s\nworktree=%s\np23_binary=%s\np23_binary_sha256=%s\np18_binary=%s\np18_binary_sha256=%s\ncorpus_manifest_sha256=%s\nscreen_manifest_sha256=%s\n' \
  'P23 independent VP8L analytic exact-cost / selected-only materialization experiment' \
  019f8321-035e-7211-8f53-987e18891c8c "$p23_branch" "$p23_base" \
  "$(git -C "$p23_root" rev-parse HEAD)" "$p23_root" "$p23_binary" "$p23_binary_sha" \
  "$p23_p18_binary" "$p23_p18_binary_sha" "$p23_manifest_sha" "$p23_screen_sha" \
  > "$p23_output/raw/binary-provenance.txt"

set +e
python3 "$p23_root/experiments/vp8l-analytic-exact-cost/summarize_phase_r.py" "$p23_output"
p23_summary_status=$?
set -e

find "$p23_output" -type f ! -name SHA256SUMS -print | sort | while IFS= read -r p23_path; do
  shasum -a 256 "$p23_path"
done > "$p23_output/SHA256SUMS"
if test "$p23_summary_status" -eq 0; then
  printf 'phase_r_gate=pass\n' > "$p23_output/status.txt"
else
  printf 'phase_r_gate=fail\n' > "$p23_output/status.txt"
fi
printf 'external_sha256sums_sha256=%s\n' \
  "$(shasum -a 256 "$p23_output/SHA256SUMS" | cut -d ' ' -f 1)" >> "$p23_output/status.txt"
exit "$p23_summary_status"
