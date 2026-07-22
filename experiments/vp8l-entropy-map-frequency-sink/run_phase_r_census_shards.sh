#!/usr/bin/env bash
set -euo pipefail

p25_root=$(cd "$(dirname "$0")/../.." && pwd)
p25_output=${1:?usage: run_phase_r_census_shards.sh OUTPUT prepare|SHARD|summarize [CORPUS]}
p25_stage=${2:?usage: run_phase_r_census_shards.sh OUTPUT prepare|SHARD|summarize [CORPUS]}
p25_corpus=${3:-/Users/lance/Desktop/webp/third_party/benchdata/clic/vp8l-lossless-exact}
p25_raw="$p25_output/raw"
p25_shards="$p25_raw/census-shards"
p25_binary=$(<"$p25_raw/p25-binary-path.txt")
p25_test=vp8l::image_writer::product_benchmark_tests::product_validation_reproducer

test "$(git -C "$p25_root" branch --show-current)" = codex/vp8l-entropy-map-frequency-sink
test -z "$(git -C "$p25_root" status --porcelain)"
test "$(shasum -a 256 "$p25_binary" | cut -d ' ' -f 1)" = \
  2d00a2699eaff1bd8e542b0e11987fa28e51a2d91662e78874cac989b2296276

case "$p25_stage" in
  prepare)
    test ! -e "$p25_shards"
    mkdir -p "$p25_shards"
    find "$p25_corpus" -maxdepth 1 -type f -name '*-m6.webp' -print | sort > "$p25_shards/inputs.tsv"
    test "$(wc -l < "$p25_shards/inputs.tsv" | tr -d ' ')" -eq 102
    p25_index=0
    while IFS= read -r p25_path; do
      p25_shard=$(printf '%02d' $((p25_index / 9)))
      p25_dir="$p25_shards/$p25_shard"
      mkdir -p "$p25_dir"
      ln -s "$p25_path" "$p25_dir/${p25_path##*/}"
      p25_index=$((p25_index + 1))
    done < "$p25_shards/inputs.tsv"
    ;;
  summarize)
    python3 "$p25_root/experiments/vp8l-entropy-map-frequency-sink/summarize_phase_r.py" "$p25_output"
    find "$p25_output" -type f ! -name SHA256SUMS -print | sort | while IFS= read -r p25_path; do
      shasum -a 256 "$p25_path"
    done > "$p25_output/SHA256SUMS"
    ;;
  [0-9]|[0-9][0-9])
    p25_shard=$(printf '%02d' "$((10#$p25_stage))")
    test -d "$p25_shards/$p25_shard"
    test ! -e "$p25_raw/candidate-census-shard-$p25_shard.tsv"
    env VP8L_PRODUCT_COMMAND=exact-cost-audit VP8L_PRODUCT_INPUT="$p25_shards/$p25_shard" \
      "$p25_binary" --exact "$p25_test" --ignored --nocapture \
      > "$p25_raw/candidate-census-shard-$p25_shard.tsv" \
      2> "$p25_raw/candidate-census-shard-$p25_shard.stderr"
    ;;
  *)
    exit 64
    ;;
esac
