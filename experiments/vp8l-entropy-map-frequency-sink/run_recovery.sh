#!/usr/bin/env bash
set -euo pipefail

p25_root=$(cd "$(dirname "$0")/../.." && pwd)
p25_output=${1:?usage: run_recovery.sh OUTPUT prepare|warmup|forward-1|reverse-2|forward-3|summarize [CORPUS]}
p25_stage=${2:?usage: run_recovery.sh OUTPUT prepare|warmup|forward-1|reverse-2|forward-3|summarize [CORPUS]}
p25_corpus=${3:-/Users/lance/Desktop/webp/third_party/benchdata/clic/vp8l-lossless-exact}
p25_binary=/private/tmp/vp8l-entropy-map-frequency-sink-p25-phase-r-manual-5cc3f96b/raw/p25-target/release/deps/webp-1efd5f53c381ab2b
p25_binary_sha=2d00a2699eaff1bd8e542b0e11987fa28e51a2d91662e78874cac989b2296276
p25_test=vp8l::image_writer::product_benchmark_tests::product_validation_reproducer
p25_raw="$p25_output/raw"
p25_screen="$p25_raw/screen-input"

test "$(git -C "$p25_root" branch --show-current)" = codex/vp8l-entropy-map-frequency-sink
test -z "$(git -C "$p25_root" status --porcelain)"
test -x "$p25_binary"
test "$(shasum -a 256 "$p25_binary" | cut -d ' ' -f 1)" = "$p25_binary_sha"

p25_run() {
  p25_layout=$1
  p25_label=$2
  test ! -e "$p25_raw/$p25_layout-$p25_label.tsv"
  test ! -e "$p25_raw/$p25_layout-$p25_label.stderr"
  env VP8L_PRODUCT_COMMAND=bench-encode VP8L_PRODUCT_INPUT="$p25_screen" \
    VP8L_PRODUCT_LAYOUT="$p25_layout" VP8L_PRODUCT_ROUND="$p25_label" \
    "$p25_binary" --exact "$p25_test" --ignored --nocapture \
    > "$p25_raw/$p25_layout-$p25_label.tsv" \
    2> "$p25_raw/$p25_layout-$p25_label.stderr"
}

case "$p25_stage" in
  prepare)
    test ! -e "$p25_output"
    mkdir -p "$p25_screen"
    find "$p25_corpus" -maxdepth 1 -type f -name '*-m6.webp' -print | sort | head -n 41 > "$p25_raw/screen-paths.tsv"
    while IFS= read -r p25_path; do
      ln -s "$p25_path" "$p25_screen/${p25_path##*/}"
      printf '%s\t%s\t%s\n' "${p25_path##*/}" "$(stat -f %z "$p25_path")" \
        "$(shasum -a 256 "$p25_path" | cut -d ' ' -f 1)"
    done < "$p25_raw/screen-paths.tsv" > "$p25_raw/screen-manifest-41.tsv"
    test "$(wc -l < "$p25_raw/screen-manifest-41.tsv" | tr -d ' ')" -eq 41
    test "$(shasum -a 256 "$p25_raw/screen-manifest-41.tsv" | cut -d ' ' -f 1)" = \
      474587feabe3178268b2eab6f7a166501d8ecc3d637a76bd412f4233dfa7b913
    printf 'task=P25\nroot_task=019f8321-035e-7211-8f53-987e18891c8c\nbinary_source_head=5cc3f96bdc4d619806f936971c001c99130ae0f8\nrunner_head=%s\nreused_binary=%s\np25_binary_sha256=%s\nscreen_manifest_sha256=474587feabe3178268b2eab6f7a166501d8ecc3d637a76bd412f4233dfa7b913\n' \
      "$(git -C "$p25_root" rev-parse HEAD)" "$p25_binary" "$p25_binary_sha" > "$p25_raw/provenance.txt"
    printf prepared > "$p25_raw/state"
    ;;
  warmup)
    test "$(<"$p25_raw/state")" = prepared
    p25_run compact warmup-a
    p25_run compact-fused-rank-sum warmup-b
    p25_run low-latency warmup-a
    p25_run low-latency-fused-rank-sum warmup-b
    printf warmup > "$p25_raw/state"
    ;;
  forward-1)
    test "$(<"$p25_raw/state")" = warmup
    p25_run compact forward-1-a
    p25_run compact-fused-rank-sum forward-1-b
    p25_run low-latency forward-1-a
    p25_run low-latency-fused-rank-sum forward-1-b
    printf forward-1 > "$p25_raw/state"
    ;;
  reverse-2)
    test "$(<"$p25_raw/state")" = forward-1
    p25_run low-latency-fused-rank-sum reverse-2-b
    p25_run low-latency reverse-2-a
    p25_run compact-fused-rank-sum reverse-2-b
    p25_run compact reverse-2-a
    printf reverse-2 > "$p25_raw/state"
    ;;
  forward-3)
    test "$(<"$p25_raw/state")" = reverse-2
    p25_run compact forward-3-a
    p25_run compact-fused-rank-sum forward-3-b
    p25_run low-latency forward-3-a
    p25_run low-latency-fused-rank-sum forward-3-b
    printf forward-3 > "$p25_raw/state"
    ;;
  summarize)
    test "$(<"$p25_raw/state")" = forward-3
    if python3 "$p25_root/experiments/vp8l-entropy-map-frequency-sink/summarize_recovery.py" "$p25_output"; then
      p25_gate=pass
    else
      p25_gate=fail
    fi
    printf 'recovery_gate=%s\n' "$p25_gate" > "$p25_output/recovery-status.txt"
    printf summarized > "$p25_raw/state"
    find "$p25_output" -type f ! -name SHA256SUMS -print | sort | while IFS= read -r p25_path; do
      shasum -a 256 "$p25_path"
    done > "$p25_output/SHA256SUMS"
    test "$p25_gate" = pass
    ;;
  *) exit 64 ;;
esac
