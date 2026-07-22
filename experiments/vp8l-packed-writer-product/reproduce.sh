#!/usr/bin/env bash
set -euo pipefail

vp8l_repo=$(cd "$(dirname "$0")/../.." && pwd)
vp8l_corpus=${1:-/Users/lance/Desktop/webp/third_party/benchdata/clic/vp8l-lossless-exact}
vp8l_oracle=${2:-/Users/lance/Desktop/webp/third_party/oracle/libwebp}
vp8l_output=${3:-/private/tmp/vp8l-packed-writer-product-reproduction}
vp8l_base_sha=0ee428dc0bee9c035f051b4ccaa846dabe394ca8
vp8l_product_sha=9435fbd0499fe76ad5579a740da518f49ebd67d0
vp8l_e36_sha=dfc0cf6f043faf329c60fa43f363b6e4bd85688a

if [[ -e "$vp8l_output" ]]; then
  echo "refusing existing output: $vp8l_output" >&2
  exit 2
fi
mkdir -p "$vp8l_output/raw"
vp8l_scratch=$(mktemp -d /private/tmp/vp8l-packed-writer-product.XXXXXX)
vp8l_cleanup() {
  python3 -c 'import shutil,sys; shutil.rmtree(sys.argv[1], ignore_errors=True)' "$vp8l_scratch"
}
trap vp8l_cleanup EXIT INT TERM HUP

for vp8l_spec in "base:$vp8l_base_sha" "product:$vp8l_product_sha" "e36:$vp8l_e36_sha"; do
  vp8l_name=${vp8l_spec%%:*}
  vp8l_sha=${vp8l_spec#*:}
  mkdir "$vp8l_scratch/$vp8l_name"
  git -C "$vp8l_repo" archive "$vp8l_sha" | tar -x -C "$vp8l_scratch/$vp8l_name"
  (
    cd "$vp8l_scratch/$vp8l_name/webp-rs"
    CARGO_TARGET_DIR="$vp8l_scratch/$vp8l_name-target" cargo test --release -p webp --lib --no-run
    CARGO_TARGET_DIR="$vp8l_scratch/$vp8l_name-target" cargo build --release -p webp
  )
done

vp8l_base_binary=$(find "$vp8l_scratch/base-target/release/deps" -type f -perm -111 -name 'webp-*' -print | head -n 1)
vp8l_product_binary=$(find "$vp8l_scratch/product-target/release/deps" -type f -perm -111 -name 'webp-*' -print | head -n 1)
vp8l_e36_binary=$(find "$vp8l_scratch/e36-target/release/deps" -type f -perm -111 -name 'webp-*' -print | head -n 1)
if [[ -z "$vp8l_base_binary" || -z "$vp8l_product_binary" || -z "$vp8l_e36_binary" ]]; then
  echo "release test binary not found" >&2
  exit 2
fi

find "$vp8l_corpus" -maxdepth 1 -type f -name '*-m6.webp' -print | sort |
  while IFS= read -r vp8l_source_path; do
    vp8l_name=$(basename "$vp8l_source_path")
    vp8l_bytes=$(stat -f '%z' "$vp8l_source_path")
    vp8l_sha=$(shasum -a 256 "$vp8l_source_path" | cut -d ' ' -f 1)
    printf '%s\t%s\t%s\n' "$vp8l_name" "$vp8l_bytes" "$vp8l_sha"
  done > "$vp8l_output/raw/corpus-manifest-102.tsv"
test "$(wc -l < "$vp8l_output/raw/corpus-manifest-102.tsv")" -eq 102

vp8l_screen_dir="$vp8l_scratch/screen-input"
mkdir "$vp8l_screen_dir"
find "$vp8l_corpus" -maxdepth 1 -type f -name '*-m6.webp' -print | sort | head -n 41 |
  while IFS= read -r vp8l_source_path; do
    ln -s "$vp8l_source_path" "$vp8l_screen_dir/$(basename "$vp8l_source_path")"
  done
find "$vp8l_screen_dir" -maxdepth 1 -type l -print | sort |
  while IFS= read -r vp8l_link_path; do
    vp8l_source_path=$(readlink "$vp8l_link_path")
    vp8l_name=$(basename "$vp8l_source_path")
    vp8l_bytes=$(stat -f '%z' "$vp8l_source_path")
    vp8l_sha=$(shasum -a 256 "$vp8l_source_path" | cut -d ' ' -f 1)
    printf '%s\t%s\t%s\n' "$vp8l_name" "$vp8l_bytes" "$vp8l_sha"
  done > "$vp8l_output/raw/screen-manifest-41.tsv"
test "$(wc -l < "$vp8l_output/raw/screen-manifest-41.tsv")" -eq 41
head -n 41 "$vp8l_output/raw/corpus-manifest-102.tsv" |
  diff - "$vp8l_output/raw/screen-manifest-41.tsv"

python3 "$vp8l_repo/tools/run-vp8l-product-benchmark.py" \
  --binary "$vp8l_product_binary" --input "$vp8l_screen_dir" \
  --generated "$vp8l_scratch/unused" --output "$vp8l_output/raw/screen-41-final" \
  --rounds 3 \
  --layouts compact-writer-control,compact,low-latency-writer-control,low-latency \
  --operations encode
python3 "$vp8l_repo/tools/run-vp8l-product-benchmark.py" \
  --binary "$vp8l_product_binary" --input "$vp8l_corpus" \
  --generated "$vp8l_scratch/unused" --output "$vp8l_output/raw/formal-102-final" \
  --rounds 5 \
  --layouts compact-writer-control,compact,low-latency-writer-control,low-latency \
  --operations encode --formal

test "$(git -C "$vp8l_oracle" rev-parse HEAD)" = 733c91e461c18cf1127c9ed0a80dccbcfed599d3
cc -O3 -Wall -Wextra -Werror -I"$vp8l_oracle/src" \
  "$vp8l_repo/tools/libwebp_vp8l_product_compare.c" "$vp8l_oracle/build/libwebp.a" \
  -o "$vp8l_scratch/libwebp-compare"
python3 "$vp8l_repo/experiments/vp8l-packed-writer-product/verify_identity.py" \
  --left-binary "$vp8l_base_binary" --right-binary "$vp8l_product_binary" \
  --left-label latest-main-0ee428dc --right-label product-9435fbd0 \
  --corpus "$vp8l_corpus" --oracle-binary "$vp8l_scratch/libwebp-compare" \
  --output "$vp8l_output/raw/identity-latest-main-product"
python3 "$vp8l_repo/experiments/vp8l-packed-writer-product/verify_identity.py" \
  --left-binary "$vp8l_product_binary" --right-binary "$vp8l_e36_binary" \
  --left-label product-9435fbd0 --right-label e36-dfc0cf6f \
  --corpus "$vp8l_corpus" --oracle-binary "$vp8l_scratch/libwebp-compare" \
  --output "$vp8l_output/raw/identity-product-e36"

python3 "$vp8l_repo/experiments/vp8l-packed-writer-product/summarize.py" "$vp8l_output"
python3 - "$vp8l_output/gate-summary.json" <<'PY'
import json
import sys

summary = json.load(open(sys.argv[1]))
for stage in ("screen", "formal"):
    for profile in ("compact", "low-latency"):
        row = summary[stage][profile]
        if row["independent_change_percent"] > -20:
            raise SystemExit(f"{stage} {profile} speed gate failed: {row}")
        if row["image_regressions"]:
            raise SystemExit(f"{stage} {profile} image regression: {row}")
for profile in ("compact", "low-latency"):
    if summary["formal"][profile]["candidate_median_seconds"] > 8.5:
        raise SystemExit(f"formal {profile} absolute target failed")
PY

vp8l_validation="$vp8l_output/raw/validation"
mkdir "$vp8l_validation"
(
  cd "$vp8l_scratch/product/webp-rs"
  CARGO_TARGET_DIR="$vp8l_scratch/validation-target" cargo test --workspace --all-targets
  CARGO_TARGET_DIR="$vp8l_scratch/validation-target" cargo test --release --workspace --all-targets
  CARGO_TARGET_DIR="$vp8l_scratch/validation-target" cargo clippy --workspace --all-targets -- -D warnings
  cargo fmt --all -- --check
  RUSTDOCFLAGS='-D warnings' CARGO_TARGET_DIR="$vp8l_scratch/validation-target" cargo doc -p webp --no-deps
  CARGO_TARGET_DIR="$vp8l_scratch/validation-target" cargo test -p webp --doc
) > "$vp8l_validation/all.log" 2>&1

find "$vp8l_output" -type f ! -name SHA256SUMS -print0 | sort -z |
  xargs -0 shasum -a 256 > "$vp8l_output/SHA256SUMS"
