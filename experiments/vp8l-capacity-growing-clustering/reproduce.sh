#!/usr/bin/env bash
set -euo pipefail

p16_repo=$(cd "$(dirname "$0")/../.." && pwd)
p16_corpus=${1:-/Users/lance/Desktop/webp/third_party/benchdata/clic/vp8l-lossless-exact}
p16_pinned=${2:-/Users/lance/Desktop/libwebp}
p16_output=${3:-/private/tmp/vp8l-capacity-growing-clustering-reproduction}
p16_manifest_sha=9feb09f469753c43864011aa6f00cfc5ee1bd48da5aac8f9b16d105890e14f86
p16_screen_sha=474587feabe3178268b2eab6f7a166501d8ecc3d637a76bd412f4233dfa7b913
p16_test=vp8l::image_writer::product_benchmark_tests::product_validation_reproducer
p16_layouts=compact-control,compact,low-latency-control,low-latency

if [[ -e "$p16_output" ]]; then
  echo "refusing existing output: $p16_output" >&2
  exit 2
fi
test "$(git -C "$p16_pinned" rev-parse HEAD)" = 733c91e461c18cf1127c9ed0a80dccbcfed599d3

p16_scratch=$(mktemp -d /private/tmp/p16-reproduce.XXXXXX)
p16_cleanup() {
  rm -f /private/tmp/webp-vp8l-product-benchmark.lock
  rm -f /private/tmp/webp-vp8l-libwebp-product-benchmark.lock
  python3 -c 'import shutil,sys; shutil.rmtree(sys.argv[1], ignore_errors=True)' "$p16_scratch"
}
trap p16_cleanup EXIT INT TERM HUP
mkdir -p "$p16_output/raw"

find "$p16_corpus" -maxdepth 1 -type f -name '*-m6.webp' -print | sort |
  while IFS= read -r p16_path; do
    printf '%s\t%s\t%s\n' "$(basename "$p16_path")" "$(stat -f '%z' "$p16_path")" \
      "$(shasum -a 256 "$p16_path" | cut -d ' ' -f 1)"
  done > "$p16_output/raw/corpus-manifest-102.tsv"
test "$(wc -l < "$p16_output/raw/corpus-manifest-102.tsv" | tr -d ' ')" -eq 102
test "$(shasum -a 256 "$p16_output/raw/corpus-manifest-102.tsv" | cut -d ' ' -f 1)" = "$p16_manifest_sha"
head -n 41 "$p16_output/raw/corpus-manifest-102.tsv" > "$p16_output/raw/screen-manifest-41.tsv"
test "$(shasum -a 256 "$p16_output/raw/screen-manifest-41.tsv" | cut -d ' ' -f 1)" = "$p16_screen_sha"

p16_target="$p16_scratch/target"
CARGO_TARGET_DIR="$p16_target" cargo test --manifest-path "$p16_repo/webp-rs/Cargo.toml" \
  --release -p webp --lib --features vp8l-capacity-growing-experiment --no-run
p16_binary=$(find "$p16_target/release/deps" -type f -perm -111 -name 'webp-*' -print)
test "$(printf '%s\n' "$p16_binary" | sed '/^$/d' | wc -l | tr -d ' ')" -eq 1
p16_binary_sha=$(shasum -a 256 "$p16_binary" | cut -d ' ' -f 1)

p16_screen="$p16_scratch/screen"
mkdir "$p16_screen"
while IFS=$'\t' read -r p16_name _; do
  ln -s "$p16_corpus/$p16_name" "$p16_screen/$p16_name"
done < "$p16_output/raw/screen-manifest-41.tsv"

mkdir "$p16_output/raw/phase-a-102-final-screen-binary"
VP8L_PRODUCT_COMMAND=p16-phase-a VP8L_PRODUCT_INPUT="$p16_corpus" \
  "$p16_binary" --exact "$p16_test" --ignored --nocapture \
  > "$p16_output/raw/phase-a-102-final-screen-binary/phase-a.tsv" \
  2> "$p16_output/raw/phase-a-102-final-screen-binary/phase-a.stderr"

python3 "$p16_repo/tools/run-vp8l-product-benchmark.py" \
  --binary "$p16_binary" --input "$p16_screen" --generated "$p16_scratch/unused" \
  --output "$p16_output/raw/screen-41-encode-final" --rounds 3 \
  --layouts "$p16_layouts" --operations encode

p16_generated="$p16_scratch/generated"
mkdir "$p16_output/raw/screen-41-correctness-final"
VP8L_PRODUCT_COMMAND=generate VP8L_PRODUCT_INPUT="$p16_screen" \
VP8L_PRODUCT_OUTPUT="$p16_generated" "$p16_binary" --exact "$p16_test" \
  --ignored --nocapture \
  > "$p16_output/raw/screen-41-correctness-final/project-generate.tsv" \
  2> "$p16_output/raw/screen-41-correctness-final/project-generate.stderr"

cmake -S "$p16_pinned" -B "$p16_scratch/libwebp-build" \
  -DCMAKE_BUILD_TYPE=Release -DBUILD_SHARED_LIBS=OFF \
  -DWEBP_BUILD_ANIM_UTILS=OFF -DWEBP_BUILD_CWEBP=OFF \
  -DWEBP_BUILD_DWEBP=OFF -DWEBP_BUILD_GIF2WEBP=OFF \
  -DWEBP_BUILD_IMG2WEBP=OFF -DWEBP_BUILD_VWEBP=OFF \
  -DWEBP_BUILD_WEBPINFO=OFF -DWEBP_BUILD_WEBPMUX=OFF \
  -DWEBP_BUILD_EXTRAS=OFF -DWEBP_BUILD_LIBWEBPMUX=OFF
cmake --build "$p16_scratch/libwebp-build" --target webp -j 4
cc -O3 -Wall -Wextra -Werror -I"$p16_pinned/src" \
  "$p16_repo/tools/libwebp_vp8l_product_compare.c" \
  "$p16_scratch/libwebp-build/libwebp.a" -o "$p16_scratch/libwebp-compare"
cc -O3 -Wall -Wextra -Werror -I"$p16_pinned/src" \
  "$p16_repo/tools/libwebp_vp8l_product_bench.c" \
  "$p16_scratch/libwebp-build/libwebp.a" -o "$p16_scratch/libwebp-bench"
find "$p16_generated" -mindepth 2 -type f -name '*.webp' -print0 | sort -z |
  xargs -0 "$p16_scratch/libwebp-compare" "$p16_generated/expected" \
  > "$p16_output/raw/screen-41-correctness-final/libwebp-compare.tsv" \
  2> "$p16_output/raw/screen-41-correctness-final/libwebp-compare.stderr"

python3 "$p16_repo/tools/run-vp8l-product-benchmark.py" \
  --binary "$p16_binary" --input "$p16_screen" --generated "$p16_generated" \
  --output "$p16_output/raw/screen-41-rust-decode-final" --rounds 3 \
  --layouts "$p16_layouts" --operations decode
python3 "$p16_repo/tools/run-vp8l-libwebp-product-benchmark.py" \
  --binary "$p16_scratch/libwebp-bench" --generated "$p16_generated" \
  --expected "$p16_generated/expected" \
  --output "$p16_output/raw/screen-41-libwebp-decode-final" --rounds 3 \
  --layouts "$p16_layouts"

printf '%s\n' \
  'Formal 102x5 was not run: Compact failed the 41-image encode screen.' \
  > "$p16_output/raw/formal-102x5-not-run.txt"
cp "$p16_repo/experiments/vp8l-capacity-growing-clustering/summarize.py" \
  "$p16_output/summarize.py"
python3 "$p16_output/summarize.py"
python3 - "$p16_output/screen-summary.json" <<'PY'
import json, sys
s = json.load(open(sys.argv[1]))
assert s["binary_sha256"]
assert s["correctness"]["project_exact"] == 246
assert s["correctness"]["pinned_c_summary"] == "oracle_summary\tmatched=246\tfailed=0"
assert s["screen"]["low-latency"]["screen_gate"]
assert not s["screen"]["compact"]["screen_gate"]
assert s["decision"] == "reject-screen"
assert not s["formal_102x5_run"]
PY
printf 'binary_sha256=%s\nformal_102x5_run=false\ndecision=reject-screen\n' \
  "$p16_binary_sha" > "$p16_output/reproduction-status.txt"
(
  cd "$p16_output"
  find . -type f ! -name SHA256SUMS -print0 | sort -z |
    xargs -0 shasum -a 256 > SHA256SUMS
  shasum -a 256 -c SHA256SUMS
)
