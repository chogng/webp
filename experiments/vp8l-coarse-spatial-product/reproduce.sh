#!/usr/bin/env bash
set -euo pipefail

repo=$(cd "$(dirname "$0")/../.." && pwd)
corpus=${1:-/Users/lance/Desktop/webp/third_party/benchdata/clic/vp8l-lossless-exact}
oracle=${2:-/Users/lance/Desktop/webp/third_party/oracle/libwebp}
generated=${3:-/private/tmp/vp8l-coarse-spatial-product-reproduction}
output=${4:-/private/tmp/vp8l-coarse-spatial-product-evidence}
source_evidence=${5:-/Users/lance/.codex/worktrees/6d6b/webp/experiments/vp8l-coarse-spatial-entropy}
screen_input=${generated}-screen-input
screen_generated=${generated}-screen-generated
base_tree=${generated}-base-52c6
base_target=${generated}-base-target

for path in "$generated" "$output" "$screen_input" "$screen_generated" \
  "$base_tree" "$base_target"; do
  if [[ -e "$path" ]]; then
    echo "refusing existing path: $path" >&2
    exit 2
  fi
done
mkdir -p "$output"

cargo test --release -p webp --lib --no-run \
  --manifest-path "$repo/webp-rs/Cargo.toml"
binary=$(find "$repo/webp-rs/target/release/deps" -type f -perm -111 \
  -name 'webp-*' -print | head -n 1)
if [[ -z "$binary" ]]; then
  echo "release webp test binary not found" >&2
  exit 2
fi

: > "$output/streams-102.unnormalized.tsv"
for index in {0..101}; do
  VP8L_PRODUCT_COMMAND=generate \
  VP8L_PRODUCT_INPUT="$corpus" \
  VP8L_PRODUCT_OUTPUT="$generated" \
  VP8L_PRODUCT_START="$index" \
  VP8L_PRODUCT_LIMIT=1 \
  "$binary" --exact \
    encoder::product_benchmark_tests::product_validation_reproducer \
    --ignored --nocapture \
    >> "$output/streams-102.unnormalized.tsv"
done
awk -F '\t' '
  BEGIN {OFS="\t"; print "stream","id","layout","bytes","rgba_hash","stream_hash","encode_ns","project_exact"}
  $1=="stream" && $2!="id" {key=$2 SUBSEP $3; if (!seen[key]++) print $0}
' "$output/streams-102.unnormalized.tsv" > "$output/streams-102.tsv"

cc -O3 -Wall -Wextra -Werror -I"$oracle/src" \
  "$repo/tools/libwebp_vp8l_product_compare.c" "$oracle/build/libwebp.a" \
  -o "$output/libwebp_vp8l_product_compare"
"$output/libwebp_vp8l_product_compare" "$generated/expected" \
  "$generated"/default/*.webp "$generated"/single/*.webp \
  "$generated"/compact/*.webp "$generated"/low-latency/*.webp \
  > "$output/oracle-408.tsv"

awk -F '\t' 'BEGIN{OFS="\t"} NR==1 {print; next}
  $2=="single" || $2=="b128-g64" {
    if ($2=="b128-g64") $2="compact"; print
  }' "$source_evidence/streams-102.tsv" > "$output/source-normalized.tsv"
awk -F '\t' 'BEGIN{OFS="\t"} NR>1 && $2=="b256-g16" {
  $2="low-latency"; print
}' "$source_evidence/streams-102-b256-g16.tsv" >> "$output/source-normalized.tsv"
awk -F '\t' '
  BEGIN {OFS="\t"; print "id","layout","bytes","rgba_hash","stream_hash","source_groups","source_map_cells","source_row_runs","source_group_switches","source_token_group_switches","identity"}
  FNR==NR {if (FNR>1) {key=$1 SUBSEP $2; bytes[key]=$3; rgba[key]=$20; hash[key]=$21; groups[key]=$7; cells[key]=$9; runs[key]=$10; switches[key]=$11; token_switches[key]=$12} next}
  $1=="stream" && ($3=="single" || $3=="compact" || $3=="low-latency") {
    key=$2 SUBSEP $3; exact=($4==bytes[key] && $5==rgba[key] && $6==hash[key]);
    print $2,$3,$4,$5,$6,groups[key],cells[key],runs[key],switches[key],token_switches[key],exact ? "exact" : "MISMATCH"
  }
' "$output/source-normalized.tsv" "$output/streams-102.tsv" \
  > "$output/stream-identity-306.tsv"

mkdir "$screen_input" "$screen_generated"
for layout in single compact low-latency; do
  mkdir "$screen_generated/$layout"
done
count=0
for input in "$corpus"/*-m6.webp; do
  if [[ "$count" -ge 41 ]]; then break; fi
  name=${input##*/}
  id=${name%-m6.webp}
  ln -s "$input" "$screen_input/$name"
  for layout in single compact low-latency; do
    ln -s "$generated/$layout/$id.webp" "$screen_generated/$layout/$id.webp"
  done
  count=$((count + 1))
done
[[ "$count" -eq 41 ]]

python3 "$repo/tools/run-vp8l-product-benchmark.py" \
  --binary "$binary" --input "$screen_input" --generated "$screen_generated" \
  --output "$output/screen-41" --rounds 3 --operations decode
python3 "$repo/tools/summarize-vp8l-product-benchmark.py" "$output/screen-41"
python3 "$repo/tools/run-vp8l-product-benchmark.py" \
  --binary "$binary" --input "$corpus" --generated "$generated" \
  --output "$output/formal-102" --rounds 5 --operations decode,encode --formal
python3 "$repo/tools/summarize-vp8l-product-benchmark.py" "$output/formal-102"

mkdir "$generated/libwebp-m6"
for input in "$corpus"/*-m6.webp; do
  name=${input##*/}
  id=${name%-m6.webp}
  ln -s "$input" "$generated/libwebp-m6/$id.webp"
done
python3 "$repo/tools/run-vp8l-product-benchmark.py" \
  --binary "$binary" --input "$corpus" --generated "$generated" \
  --output "$output/comparative-decode-102" --rounds 5 \
  --layouts single,default,compact,low-latency,libwebp-m6 \
  --operations decode --formal
python3 "$repo/tools/summarize-vp8l-product-benchmark.py" \
  "$output/comparative-decode-102"

cc -O3 -Wall -Wextra -Werror -I"$oracle/src" \
  "$repo/tools/libwebp_vp8l_product_bench.c" "$oracle/build/libwebp.a" \
  -o "$output/libwebp_vp8l_product_bench"
python3 "$repo/tools/run-vp8l-libwebp-product-benchmark.py" \
  --binary "$output/libwebp_vp8l_product_bench" --generated "$generated" \
  --expected "$generated/expected" --output "$output/libwebp-decode-102" \
  --layouts libwebp-m6,default,compact,low-latency --rounds 5 --formal
python3 "$repo/tools/summarize-vp8l-product-benchmark.py" \
  "$output/libwebp-decode-102"

mkdir "$base_tree"
git -C "$repo" archive 52c6b8fc64cd86b4fccd0f30fb996d825a6dd2ec |
  tar -x -C "$base_tree"
CARGO_TARGET_DIR="$base_target" cargo build --release -p webp \
  --example vp8l_color_transform_reproducer \
  --manifest-path "$base_tree/webp-rs/Cargo.toml"
cargo build --release -p webp --example vp8l_color_transform_reproducer \
  --manifest-path "$repo/webp-rs/Cargo.toml"
"$base_target/release/examples/vp8l_color_transform_reproducer" scan \
  "$corpus" "$oracle/build/dwebp" > "$output/default-before-102.tsv"
"$repo/webp-rs/target/release/examples/vp8l_color_transform_reproducer" scan \
  "$corpus" "$oracle/build/dwebp" > "$output/default-after-102.tsv"
cmp "$output/default-before-102.tsv" "$output/default-after-102.tsv"
