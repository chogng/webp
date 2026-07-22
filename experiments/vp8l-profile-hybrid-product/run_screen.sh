#!/bin/sh
set -eu

p20_root=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
p20_output=${1:?usage: run_screen.sh OUTPUT_DIR PRODUCT_BINARY [CORPUS] [PINNED_LIBWEBP] [EXPECTED_SHA256]}
p20_binary=${2:?usage: run_screen.sh OUTPUT_DIR PRODUCT_BINARY [CORPUS] [PINNED_LIBWEBP] [EXPECTED_SHA256]}
p20_corpus=${3:-/Users/lance/Desktop/webp/third_party/benchdata/clic/vp8l-lossless-exact}
p20_pinned=${4:-/Users/lance/Desktop/libwebp}
p20_expected_sha=${5:-9aa8fa08fb2288335a98ff4a3d5f64a5a7922372f7c4c1b1212ed98b9b1a29f8}
p20_test=vp8l::image_writer::product_benchmark_tests::product_validation_reproducer
p20_layouts=compact-control,compact,low-latency-control,low-latency
p20_screen_sha=474587feabe3178268b2eab6f7a166501d8ecc3d637a76bd412f4233dfa7b913

test "$(shasum -a 256 "$p20_binary" | cut -d ' ' -f 1)" = "$p20_expected_sha"
test "$(git -C "$p20_pinned" rev-parse HEAD)" = 733c91e461c18cf1127c9ed0a80dccbcfed599d3
mkdir "$p20_output"
mkdir "$p20_output/raw"
mkdir "$p20_output/raw/screen-input"
find "$p20_corpus" -maxdepth 1 -type f -name '*-m6.webp' -print | sort |
while IFS= read -r p20_path; do
  p20_name=${p20_path##*/}
  p20_bytes=$(stat -f %z "$p20_path")
  p20_sha=$(shasum -a 256 "$p20_path" | cut -d ' ' -f 1)
  printf '%s\t%s\t%s\n' "$p20_name" "$p20_bytes" "$p20_sha"
done | head -n 41 > "$p20_output/raw/screen-manifest-41.tsv"
test "$(shasum -a 256 "$p20_output/raw/screen-manifest-41.tsv" | cut -d ' ' -f 1)" = "$p20_screen_sha"
while IFS="$(printf '\t')" read -r p20_name _; do
  ln -s "$p20_corpus/$p20_name" "$p20_output/raw/screen-input/$p20_name"
done < "$p20_output/raw/screen-manifest-41.tsv"

python3 "$p20_root/tools/run-vp8l-product-benchmark.py" \
  --binary "$p20_binary" --input "$p20_output/raw/screen-input" \
  --generated "$p20_output/raw/unused" --output "$p20_output/raw/screen-encode" \
  --rounds 3 --layouts "$p20_layouts" --operations encode

env VP8L_PRODUCT_COMMAND=generate VP8L_PRODUCT_INPUT="$p20_output/raw/screen-input" \
  VP8L_PRODUCT_OUTPUT="$p20_output/raw/generated" \
  "$p20_binary" --exact "$p20_test" --ignored --nocapture \
  > "$p20_output/raw/project-generate.tsv" 2> "$p20_output/raw/project-generate.stderr"

cmake -S "$p20_pinned" -B "$p20_output/raw/libwebp-build" \
  -DCMAKE_BUILD_TYPE=Release -DBUILD_SHARED_LIBS=OFF \
  -DWEBP_BUILD_ANIM_UTILS=OFF -DWEBP_BUILD_CWEBP=OFF \
  -DWEBP_BUILD_DWEBP=OFF -DWEBP_BUILD_GIF2WEBP=OFF \
  -DWEBP_BUILD_IMG2WEBP=OFF -DWEBP_BUILD_VWEBP=OFF \
  -DWEBP_BUILD_WEBPINFO=OFF -DWEBP_BUILD_WEBPMUX=OFF \
  -DWEBP_BUILD_EXTRAS=OFF -DWEBP_BUILD_LIBWEBPMUX=OFF \
  > "$p20_output/raw/libwebp-cmake.log" 2>&1
cmake --build "$p20_output/raw/libwebp-build" --target webp -j 4 \
  > "$p20_output/raw/libwebp-build.log" 2>&1
cc -O3 -Wall -Wextra -Werror -I"$p20_pinned/src" \
  "$p20_root/tools/libwebp_vp8l_product_compare.c" \
  "$p20_output/raw/libwebp-build/libwebp.a" -o "$p20_output/raw/libwebp-compare"
cc -O3 -Wall -Wextra -Werror -I"$p20_pinned/src" \
  "$p20_root/tools/libwebp_vp8l_product_bench.c" \
  "$p20_output/raw/libwebp-build/libwebp.a" -o "$p20_output/raw/libwebp-bench"
find "$p20_output/raw/generated" -mindepth 2 -type f -name '*.webp' -print0 | sort -z |
xargs -0 "$p20_output/raw/libwebp-compare" "$p20_output/raw/generated/expected" \
  > "$p20_output/raw/pinned-compare.tsv" 2> "$p20_output/raw/pinned-compare.stderr"

python3 "$p20_root/tools/run-vp8l-product-benchmark.py" \
  --binary "$p20_binary" --input "$p20_output/raw/screen-input" \
  --generated "$p20_output/raw/generated" --output "$p20_output/raw/screen-rust-decode" \
  --rounds 3 --layouts "$p20_layouts" --operations decode
python3 "$p20_root/tools/run-vp8l-libwebp-product-benchmark.py" \
  --binary "$p20_output/raw/libwebp-bench" --generated "$p20_output/raw/generated" \
  --expected "$p20_output/raw/generated/expected" \
  --output "$p20_output/raw/screen-c-decode" --rounds 3 --layouts "$p20_layouts"

python3 "$p20_root/experiments/vp8l-profile-hybrid-product/summarize_screen.py" \
  --encode "$p20_output/raw/screen-encode" \
  --rust-decode "$p20_output/raw/screen-rust-decode" \
  --c-decode "$p20_output/raw/screen-c-decode" \
  --project-generate "$p20_output/raw/project-generate.tsv" \
  --pinned-compare "$p20_output/raw/pinned-compare.tsv" \
  --binary-sha256 "$p20_expected_sha" --output "$p20_output/screen-summary.json"
printf 'screen_pass=true\nbinary_sha256=%s\n' "$p20_expected_sha" > "$p20_output/status.txt"
