#!/usr/bin/env bash
set -euo pipefail

p24_root=$(cd "$(dirname "$0")/../.." && pwd)
p24_output=${1:?usage: run_phase_r.sh OUTPUT_DIR [CORPUS] [P18_WORKTREE]}
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
test -z "$(git -C "$p24_p18" status --porcelain)"
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
mkdir "$p24_output/raw/screen-input"
while IFS=$'\t' read -r p24_name _; do
  ln -s "$p24_corpus/$p24_name" "$p24_output/raw/screen-input/$p24_name"
done < "$p24_output/raw/screen-manifest-41.tsv"

CARGO_TARGET_DIR="$p24_output/raw/p24-target" cargo test \
  --manifest-path "$p24_root/webp-rs/Cargo.toml" -p webp --lib --release --no-run \
  > "$p24_output/raw/p24-build.log" 2>&1
p24_binary=$(find "$p24_output/raw/p24-target/release/deps" -maxdepth 1 -type f \
  -perm -111 -name 'webp-*' -print)
test "$(printf '%s\n' "$p24_binary" | sed '/^$/d' | wc -l | tr -d ' ')" -eq 1
p24_binary_sha=$(shasum -a 256 "$p24_binary" | cut -d ' ' -f 1)
"$p24_binary" --list "$p24_test" > "$p24_output/raw/final-binary-filter.txt"
grep -Fx "$p24_test: test" "$p24_output/raw/final-binary-filter.txt"
"$p24_binary" vp8l::image_writer --nocapture \
  > "$p24_output/raw/mechanism-tests.log" 2>&1

env VP8L_PRODUCT_COMMAND=exact-cost-audit VP8L_PRODUCT_INPUT="$p24_corpus" \
  "$p24_binary" --exact "$p24_test" --ignored --nocapture \
  > "$p24_output/raw/candidate-audit-102.tsv" \
  2> "$p24_output/raw/candidate-audit-102.stderr"

CARGO_TARGET_DIR="$p24_output/raw/p18-target" cargo test \
  --manifest-path "$p24_p18/webp-rs/Cargo.toml" -p webp --lib --release \
  --features vp8l-profile-hybrid-experiment --no-run \
  > "$p24_output/raw/p18-build.log" 2>&1
p24_p18_binary=$(find "$p24_output/raw/p18-target/release/deps" -maxdepth 1 -type f \
  -perm -111 -name 'webp-*' -print)
test "$(printf '%s\n' "$p24_p18_binary" | sed '/^$/d' | wc -l | tr -d ' ')" -eq 1
p24_p18_binary_sha=$(shasum -a 256 "$p24_p18_binary" | cut -d ' ' -f 1)

for p24_profile in compact low-latency; do
  if test "$p24_profile" = compact; then
    p24_o_layout=compact-analytic
    p24_b_layout=compact-rank-sum
  else
    p24_o_layout=low-latency-analytic
    p24_b_layout=low-latency-rank-sum
  fi
  for p24_variant in a public; do
    env VP8L_PRODUCT_COMMAND=bench-encode VP8L_PRODUCT_INPUT="$p24_corpus" \
      VP8L_PRODUCT_LAYOUT="$p24_profile" VP8L_PRODUCT_ROUND="$p24_variant-identity" \
      "$p24_binary" --exact "$p24_test" --ignored --nocapture \
      > "$p24_output/raw/$p24_variant-$p24_profile.tsv" \
      2> "$p24_output/raw/$p24_variant-$p24_profile.stderr"
  done
  env VP8L_PRODUCT_COMMAND=bench-encode VP8L_PRODUCT_INPUT="$p24_corpus" \
    VP8L_PRODUCT_LAYOUT="$p24_o_layout" VP8L_PRODUCT_ROUND=o-identity \
    "$p24_binary" --exact "$p24_test" --ignored --nocapture \
    > "$p24_output/raw/o-$p24_profile.tsv" \
    2> "$p24_output/raw/o-$p24_profile.stderr"
  env VP8L_PRODUCT_COMMAND=bench-encode VP8L_PRODUCT_INPUT="$p24_corpus" \
    VP8L_PRODUCT_LAYOUT="$p24_b_layout" VP8L_PRODUCT_ROUND=b-identity \
    "$p24_binary" --exact "$p24_test" --ignored --nocapture \
    > "$p24_output/raw/b-$p24_profile.tsv" \
    2> "$p24_output/raw/b-$p24_profile.stderr"
  env VP8L_PRODUCT_COMMAND=bench-encode VP8L_PRODUCT_INPUT="$p24_corpus" \
    VP8L_PRODUCT_LAYOUT="$p24_profile" VP8L_PRODUCT_ROUND=p18-oracle \
    "$p24_p18_binary" --exact "$p24_test" --ignored --nocapture \
    > "$p24_output/raw/p18-$p24_profile.tsv" \
    2> "$p24_output/raw/p18-$p24_profile.stderr"
done

printf 'task=%s\nroot_task=%s\nbranch=%s\nbase=%s\nhead=%s\nworktree=%s\np24_binary=%s\np24_binary_sha256=%s\np18_binary=%s\np18_binary_sha256=%s\ncorpus_manifest_sha256=%s\nscreen_manifest_sha256=%s\n' \
  'P24 independent VP8L allocation-free rank-sum exact-cost experiment' \
  019f8321-035e-7211-8f53-987e18891c8c "$p24_branch" "$p24_base" \
  "$(git -C "$p24_root" rev-parse HEAD)" "$p24_root" "$p24_binary" "$p24_binary_sha" \
  "$p24_p18_binary" "$p24_p18_binary_sha" "$p24_manifest_sha" "$p24_screen_sha" \
  > "$p24_output/raw/binary-provenance.txt"

# Mechanism, census, corpus identity, and selector gates reject before timing.
python3 "$p24_root/experiments/vp8l-rank-sum-exact-cost/summarize_phase_r.py" \
  "$p24_output" mechanism

p24_measure() {
  p24_sequence=$1
  p24_round=$2
  p24_profile=$3
  p24_variant=$4
  p24_layout=$p24_profile
  if test "$p24_variant" = b; then
    p24_layout="$p24_profile-rank-sum"
  fi
  p24_stem=$(printf '%02d-%s-%s-%s' \
    "$p24_sequence" "$p24_round" "$p24_profile" "$p24_variant")
  env VP8L_PRODUCT_COMMAND=bench-encode \
    VP8L_PRODUCT_INPUT="$p24_output/raw/screen-input" \
    VP8L_PRODUCT_LAYOUT="$p24_layout" VP8L_PRODUCT_ROUND="$p24_round" \
    "$p24_binary" --exact "$p24_test" --ignored --nocapture \
    > "$p24_output/raw/$p24_stem.tsv" \
    2> "$p24_output/raw/$p24_stem.stderr"
}

# Exactly one unscored warmup in forward profile/variant order.
p24_measure 1 warmup compact a
p24_measure 2 warmup compact b
p24_measure 3 warmup low-latency a
p24_measure 4 warmup low-latency b
# The only valid recovery sample: three retained F/R/F rounds.
p24_measure 5 r1 compact a
p24_measure 6 r1 compact b
p24_measure 7 r1 low-latency a
p24_measure 8 r1 low-latency b
p24_measure 9 r2 low-latency b
p24_measure 10 r2 low-latency a
p24_measure 11 r2 compact b
p24_measure 12 r2 compact a
p24_measure 13 r3 compact a
p24_measure 14 r3 compact b
p24_measure 15 r3 low-latency a
p24_measure 16 r3 low-latency b

set +e
python3 "$p24_root/experiments/vp8l-rank-sum-exact-cost/summarize_phase_r.py" \
  "$p24_output" recovery
p24_summary_status=$?
set -e

find "$p24_output" -type f ! -name SHA256SUMS -print | sort | while IFS= read -r p24_path; do
  shasum -a 256 "$p24_path"
done > "$p24_output/SHA256SUMS"
if test "$p24_summary_status" -eq 0; then
  printf 'phase_r_gate=pass\n' > "$p24_output/status.txt"
else
  printf 'phase_r_gate=fail\n' > "$p24_output/status.txt"
fi
printf 'external_sha256sums_sha256=%s\n' \
  "$(shasum -a 256 "$p24_output/SHA256SUMS" | cut -d ' ' -f 1)" >> "$p24_output/status.txt"
exit "$p24_summary_status"
