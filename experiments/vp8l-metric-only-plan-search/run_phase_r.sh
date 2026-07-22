#!/usr/bin/env bash
set -euo pipefail

p22_root=$(cd "$(dirname "$0")/../.." && pwd)
p22_output=${1:?usage: run_phase_r.sh OUTPUT_DIR [CORPUS] [P18_WORKTREE]}
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
mkdir "$p22_output/raw/screen-input"
while IFS=$'\t' read -r p22_name _; do
  ln -s "$p22_corpus/$p22_name" "$p22_output/raw/screen-input/$p22_name"
done < "$p22_output/raw/screen-manifest-41.tsv"

CARGO_TARGET_DIR="$p22_output/raw/p22-target" cargo test \
  --manifest-path "$p22_root/webp-rs/Cargo.toml" -p webp --lib --release --no-run \
  > "$p22_output/raw/p22-build.log" 2>&1
p22_binary=$(find "$p22_output/raw/p22-target/release/deps" -maxdepth 1 -type f \
  -perm -111 -name 'webp-*' -print)
test "$(printf '%s\n' "$p22_binary" | sed '/^$/d' | wc -l | tr -d ' ')" -eq 1
p22_binary_sha=$(shasum -a 256 "$p22_binary" | cut -d ' ' -f 1)
"$p22_binary" --list "$p22_test" > "$p22_output/raw/final-binary-filter.txt"
grep -Fx "$p22_test: test" "$p22_output/raw/final-binary-filter.txt"
"$p22_binary" vp8l::image_writer --nocapture \
  > "$p22_output/raw/mechanism-tests.log" 2>&1

env VP8L_PRODUCT_COMMAND=metric-phase-r VP8L_PRODUCT_INPUT="$p22_corpus" \
  "$p22_binary" --exact "$p22_test" --ignored --nocapture \
  > "$p22_output/raw/phase-r-102.tsv" 2> "$p22_output/raw/phase-r-102.stderr"
env VP8L_PRODUCT_COMMAND=metric-census VP8L_PRODUCT_INPUT="$p22_corpus" \
  "$p22_binary" --exact "$p22_test" --ignored --nocapture \
  > "$p22_output/raw/census-102.tsv" 2> "$p22_output/raw/census-102.stderr"

CARGO_TARGET_DIR="$p22_output/raw/p18-target" cargo test \
  --manifest-path "$p22_p18/webp-rs/Cargo.toml" -p webp --lib --release \
  --features vp8l-profile-hybrid-experiment --no-run \
  > "$p22_output/raw/p18-build.log" 2>&1
p22_p18_binary=$(find "$p22_output/raw/p18-target/release/deps" -maxdepth 1 -type f \
  -perm -111 -name 'webp-*' -print)
test "$(printf '%s\n' "$p22_p18_binary" | sed '/^$/d' | wc -l | tr -d ' ')" -eq 1
p22_p18_binary_sha=$(shasum -a 256 "$p22_p18_binary" | cut -d ' ' -f 1)
for p22_profile in compact low-latency; do
  env VP8L_PRODUCT_COMMAND=bench-encode VP8L_PRODUCT_INPUT="$p22_corpus" \
    VP8L_PRODUCT_LAYOUT="$p22_profile" VP8L_PRODUCT_ROUND=p18-oracle \
    "$p22_p18_binary" --exact "$p22_test" --ignored --nocapture \
    > "$p22_output/raw/p18-$p22_profile.tsv" \
    2> "$p22_output/raw/p18-$p22_profile.stderr"
done

p22_measure() {
  p22_sequence=$1
  p22_round=$2
  p22_profile=$3
  p22_variant=$4
  p22_stem=$(printf '%02d-%s-%s-%s' \
    "$p22_sequence" "$p22_round" "$p22_profile" "$p22_variant")
  env VP8L_PRODUCT_COMMAND=metric-bench \
    VP8L_PRODUCT_INPUT="$p22_output/raw/screen-input" \
    VP8L_METRIC_PROFILE="$p22_profile" VP8L_METRIC_VARIANT="$p22_variant" \
    VP8L_PRODUCT_ROUND="$p22_round" \
    "$p22_binary" --exact "$p22_test" --ignored --nocapture \
    > "$p22_output/raw/$p22_stem.tsv" \
    2> "$p22_output/raw/$p22_stem.stderr"
}

# Exactly one unscored warmup round in forward variant/profile order.
p22_measure 1 warmup compact a
p22_measure 2 warmup compact b
p22_measure 3 warmup low-latency a
p22_measure 4 warmup low-latency b
# Three retained rounds in forward / reverse / forward order.
p22_measure 5 r1 compact a
p22_measure 6 r1 compact b
p22_measure 7 r1 low-latency a
p22_measure 8 r1 low-latency b
p22_measure 9 r2 low-latency b
p22_measure 10 r2 low-latency a
p22_measure 11 r2 compact b
p22_measure 12 r2 compact a
p22_measure 13 r3 compact a
p22_measure 14 r3 compact b
p22_measure 15 r3 low-latency a
p22_measure 16 r3 low-latency b

printf 'task=%s\nroot_task=%s\nbranch=%s\nbase=%s\nhead=%s\nworktree=%s\np22_binary=%s\np22_binary_sha256=%s\np18_binary=%s\np18_binary_sha256=%s\ncorpus_manifest_sha256=%s\nscreen_manifest_sha256=%s\n' \
  'P22 independent VP8L metric-only search / final-plan materialization recovery experiment' \
  019f8321-035e-7211-8f53-987e18891c8c "$p22_branch" "$p22_base" \
  "$(git -C "$p22_root" rev-parse HEAD)" "$p22_root" "$p22_binary" "$p22_binary_sha" \
  "$p22_p18_binary" "$p22_p18_binary_sha" "$p22_manifest_sha" "$p22_screen_sha" \
  > "$p22_output/raw/binary-provenance.txt"

set +e
env PYTHONPYCACHEPREFIX=/private/tmp/vp8l-metric-only-plan-search-p22-pycache \
  python3 "$p22_root/experiments/vp8l-metric-only-plan-search/summarize_phase_r.py" \
  "$p22_output"
p22_summary_status=$?
set -e

shasum -a 256 \
  "$p22_output/raw/corpus-manifest-102.tsv" \
  "$p22_output/raw/screen-manifest-41.tsv" \
  "$p22_output/raw/phase-r-102.tsv" \
  "$p22_output/raw/census-102.tsv" \
  "$p22_output/raw/p18-compact.tsv" \
  "$p22_output/raw/p18-low-latency.tsv" \
  "$p22_output/raw/binary-provenance.txt" \
  "$p22_output/phase-r-summary.json" \
  "$p22_output/recovery-summary.json" > "$p22_output/SHA256SUMS"
if test "$p22_summary_status" -eq 0; then
  printf 'phase_r_gate=pass\n' > "$p22_output/status.txt"
else
  printf 'phase_r_gate=fail\n' > "$p22_output/status.txt"
fi
printf 'external_sha256sums_sha256=%s\n' \
  "$(shasum -a 256 "$p22_output/SHA256SUMS" | cut -d ' ' -f 1)" >> \
  "$p22_output/status.txt"
exit "$p22_summary_status"
