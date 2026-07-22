#!/usr/bin/env bash
set -euo pipefail

repo=$(cd "$(dirname "$0")/../.." && pwd)
corpus=${1:-/Users/lance/Desktop/webp/third_party/benchdata/clic/vp8l-lossless-exact}
oracle=${2:-/Users/lance/Desktop/webp/third_party/oracle/libwebp}
output=${3:-/private/tmp/vp8l-exact-cost-product-reproduction}
base_sha=130aa1f347ae1193463f35205b5bd98b4031bc7c
product_sha=6ed10e559e82873d89606943e183d5432634b1a1
p09_sha=a89e0f73d6f54f87df6a25d866955591c208dc92
test_name=encoder::product_benchmark_tests::product_validation_reproducer

if [[ -e "$output" ]]; then
  echo "refusing existing output: $output" >&2
  exit 2
fi
mkdir -p "$output"
scratch=$(mktemp -d /private/tmp/vp8l-exact-cost-product.XXXXXX)
cleanup() {
  python3 -c 'import shutil,sys; shutil.rmtree(sys.argv[1], ignore_errors=True)' "$scratch"
}
trap cleanup EXIT INT TERM HUP

for name in base product p09; do
  mkdir "$scratch/$name"
done
git -C "$repo" archive "$base_sha" | tar -x -C "$scratch/base"
git -C "$repo" archive "$product_sha" | tar -x -C "$scratch/product"
git -C "$repo" archive "$p09_sha" | tar -x -C "$scratch/p09"

for name in base product p09; do
  (
    cd "$scratch/$name/webp-rs"
    CARGO_TARGET_DIR="$scratch/$name-target" cargo test --release -p webp --lib --no-run
    CARGO_TARGET_DIR="$scratch/$name-target" cargo build --release -p webp
  )
done
base_binary=$(find "$scratch/base-target/release/deps" -type f -perm -111 -name 'webp-*' -print | head -n 1)
product_binary=$(find "$scratch/product-target/release/deps" -type f -perm -111 -name 'webp-*' -print | head -n 1)
p09_binary=$(find "$scratch/p09-target/release/deps" -type f -perm -111 -name 'webp-*' -print | head -n 1)
if [[ -z "$base_binary" || -z "$product_binary" || -z "$p09_binary" ]]; then
  echo "release test binary not found" >&2
  exit 2
fi

VP8L_PRODUCT_COMMAND=audit-exact \
VP8L_PRODUCT_INPUT="$corpus" \
  "$product_binary" --exact "$test_name" --ignored --nocapture \
  > "$output/exact-audit-102.tsv"

screen_input="$scratch/screen-input"
python3 - "$corpus" "$screen_input" <<'PY'
from pathlib import Path
import sys
source = Path(sys.argv[1])
target = Path(sys.argv[2])
target.mkdir()
files = sorted(source.glob("*-m6.webp"))[:41]
if len(files) != 41:
    raise SystemExit(f"expected 41 screen inputs, found {len(files)}")
for path in files:
    (target / path.name).symlink_to(path)
PY

python3 "$repo/tools/run-vp8l-product-benchmark.py" \
  --binary "$product_binary" --input "$screen_input" --generated "$scratch/unused" \
  --output "$output/screen-compact" --rounds 3 \
  --layouts compact-control,compact --operations encode
python3 "$repo/tools/run-vp8l-product-benchmark.py" \
  --binary "$product_binary" --input "$screen_input" --generated "$scratch/unused" \
  --output "$output/screen-low-latency" --rounds 3 \
  --layouts low-latency-control,low-latency --operations encode

python3 - "$repo/experiments/vp8l-exact-cost-product/summarize.py" "$output" <<'PY'
from pathlib import Path
import importlib.util
import sys
spec = importlib.util.spec_from_file_location("summary", sys.argv[1])
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
root = Path(sys.argv[2])
for profile, baseline, candidate in (
    ("compact", "compact-control", "compact"),
    ("low-latency", "low-latency-control", "low-latency"),
):
    item = module.pair(root / f"screen-{profile}", baseline, candidate)
    if item["independent_ratio_percent"] > -25:
        raise SystemExit(f"{profile} screen gate failed: {item}")
PY

python3 "$repo/tools/run-vp8l-product-benchmark.py" \
  --binary "$product_binary" --input "$corpus" --generated "$scratch/unused" \
  --output "$output/formal-102" --rounds 5 \
  --layouts compact-control,compact,low-latency-control,low-latency \
  --operations encode --formal

cc -O3 -Wall -Wextra -Werror -I"$oracle/src" \
  "$repo/tools/libwebp_vp8l_product_compare.c" "$oracle/build/libwebp.a" \
  -o "$scratch/libwebp-compare"
python3 "$repo/experiments/vp8l-exact-cost-product/verify_identity.py" \
  --base-binary "$base_binary" --product-binary "$product_binary" \
  --p09-binary "$p09_binary" --corpus "$corpus" \
  --oracle-binary "$scratch/libwebp-compare" --output "$scratch/identity"
cp "$scratch/identity/identity-306.tsv" "$output/identity-306.tsv"
cp "$scratch/identity/oracle-306.tsv" "$output/oracle-306.tsv"

base_rlib=$(find "$scratch/base-target/release/deps" -name 'libwebp-*.rlib' -print | head -n 1)
product_rlib=$(find "$scratch/product-target/release/deps" -name 'libwebp-*.rlib' -print | head -n 1)
base_rlib_bytes=$(stat -f '%z' "$base_rlib")
product_rlib_bytes=$(stat -f '%z' "$product_rlib")
base_test_bytes=$(stat -f '%z' "$base_binary")
product_test_bytes=$(stat -f '%z' "$product_binary")
{
  printf 'artifact\tbase_bytes\tproduct_bytes\tdelta_bytes\tdelta_percent\n'
  printf 'release_rlib\t%s\t%s\t%s\t%s\n' \
    "$base_rlib_bytes" "$product_rlib_bytes" \
    "$(( product_rlib_bytes - base_rlib_bytes ))" \
    "$(awk -v before="$base_rlib_bytes" -v after="$product_rlib_bytes" 'BEGIN {printf "%.3f", 100 * (after / before - 1)}')"
  printf 'release_test_binary\t%s\t%s\t%s\t%s\n' \
    "$base_test_bytes" "$product_test_bytes" \
    "$(( product_test_bytes - base_test_bytes ))" \
    "$(awk -v before="$base_test_bytes" -v after="$product_test_bytes" 'BEGIN {printf "%.3f", 100 * (after / before - 1)}')"
} > "$output/binary-delta.tsv"

python3 "$repo/experiments/vp8l-exact-cost-product/summarize.py" "$output"
find "$output" -type f ! -name SHA256SUMS -print0 | sort -z | \
  xargs -0 shasum -a 256 > "$output/SHA256SUMS"
