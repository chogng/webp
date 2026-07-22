#!/usr/bin/env bash
set -euo pipefail

p18_repo=$(cd "$(dirname "$0")/../.." && pwd)
p18_corpus=${1:-/Users/lance/Desktop/webp/third_party/benchdata/clic/vp8l-lossless-exact}
p18_pinned=${2:-/Users/lance/Desktop/libwebp}
p18_output=${3:-/private/tmp/vp8l-profile-hybrid-clustering-reproduction}
p18_manifest_sha=9feb09f469753c43864011aa6f00cfc5ee1bd48da5aac8f9b16d105890e14f86
p18_screen_sha=474587feabe3178268b2eab6f7a166501d8ecc3d637a76bd412f4233dfa7b913
p18_reference_binary_sha=05b8421c86f3286667c5cffef35ffc2bff77f68d944063a954793e2b870e64c9
p18_test=vp8l::image_writer::product_benchmark_tests::product_validation_reproducer
p18_layouts=compact-control,compact,low-latency-control,low-latency

if [[ -e "$p18_output" ]]; then
  echo "refusing existing output: $p18_output" >&2
  exit 2
fi
test "$(git -C "$p18_pinned" rev-parse HEAD)" = 733c91e461c18cf1127c9ed0a80dccbcfed599d3

p18_scratch=$(mktemp -d /private/tmp/p18-reproduce.XXXXXX)
p18_cleanup() {
  rm -f /private/tmp/webp-vp8l-product-benchmark.lock
  rm -f /private/tmp/webp-vp8l-libwebp-product-benchmark.lock
  python3 -c 'import shutil,sys; shutil.rmtree(sys.argv[1], ignore_errors=True)' "$p18_scratch"
}
trap p18_cleanup EXIT INT TERM HUP
mkdir -p "$p18_output/raw"

find "$p18_corpus" -maxdepth 1 -type f -name '*-m6.webp' -print | sort |
  while IFS= read -r p18_path; do
    printf '%s\t%s\t%s\n' "$(basename "$p18_path")" "$(stat -f '%z' "$p18_path")" \
      "$(shasum -a 256 "$p18_path" | cut -d ' ' -f 1)"
  done > "$p18_output/raw/corpus-manifest-102.tsv"
test "$(wc -l < "$p18_output/raw/corpus-manifest-102.tsv" | tr -d ' ')" -eq 102
test "$(shasum -a 256 "$p18_output/raw/corpus-manifest-102.tsv" | cut -d ' ' -f 1)" = "$p18_manifest_sha"
head -n 41 "$p18_output/raw/corpus-manifest-102.tsv" > "$p18_output/raw/screen-manifest-41.tsv"
test "$(shasum -a 256 "$p18_output/raw/screen-manifest-41.tsv" | cut -d ' ' -f 1)" = "$p18_screen_sha"

p18_target="$p18_scratch/candidate-target"
CARGO_TARGET_DIR="$p18_target" cargo test --manifest-path "$p18_repo/webp-rs/Cargo.toml" \
  --release -p webp --lib --features vp8l-profile-hybrid-experiment --no-run
p18_binary=$(find "$p18_target/release/deps" -type f -perm -111 -name 'webp-*' -print)
test "$(printf '%s\n' "$p18_binary" | sed '/^$/d' | wc -l | tr -d ' ')" -eq 1
p18_binary_sha=$(shasum -a 256 "$p18_binary" | cut -d ' ' -f 1)
"$p18_binary" --list "$p18_test" > "$p18_output/raw/final-binary-filter.txt"
grep -Fx "$p18_test: test" "$p18_output/raw/final-binary-filter.txt"

mkdir -p "$p18_output/raw/phase-a-102-final-binary"
VP8L_PRODUCT_COMMAND=p18-phase-a VP8L_PRODUCT_INPUT="$p18_corpus" \
  "$p18_binary" --exact "$p18_test" --ignored --nocapture \
  > "$p18_output/raw/phase-a-102-final-binary/phase-a.tsv" \
  2> "$p18_output/raw/phase-a-102-final-binary/phase-a.stderr"

p18_screen="$p18_scratch/screen"
mkdir "$p18_screen"
while IFS=$'\t' read -r p18_name _; do
  ln -s "$p18_corpus/$p18_name" "$p18_screen/$p18_name"
done < "$p18_output/raw/screen-manifest-41.tsv"

python3 "$p18_repo/tools/run-vp8l-product-benchmark.py" \
  --binary "$p18_binary" --input "$p18_screen" --generated "$p18_scratch/unused" \
  --output "$p18_output/raw/screen-41-encode-final" --rounds 3 \
  --layouts "$p18_layouts" --operations encode

p18_generated="$p18_scratch/screen-generated"
mkdir "$p18_output/raw/screen-41-correctness-final"
VP8L_PRODUCT_COMMAND=generate VP8L_PRODUCT_INPUT="$p18_screen" \
VP8L_PRODUCT_OUTPUT="$p18_generated" "$p18_binary" --exact "$p18_test" \
  --ignored --nocapture \
  > "$p18_output/raw/screen-41-correctness-final/project-generate.tsv" \
  2> "$p18_output/raw/screen-41-correctness-final/project-generate.stderr"

cmake -S "$p18_pinned" -B "$p18_scratch/libwebp-build" \
  -DCMAKE_BUILD_TYPE=Release -DBUILD_SHARED_LIBS=OFF \
  -DWEBP_BUILD_ANIM_UTILS=OFF -DWEBP_BUILD_CWEBP=OFF \
  -DWEBP_BUILD_DWEBP=OFF -DWEBP_BUILD_GIF2WEBP=OFF \
  -DWEBP_BUILD_IMG2WEBP=OFF -DWEBP_BUILD_VWEBP=OFF \
  -DWEBP_BUILD_WEBPINFO=OFF -DWEBP_BUILD_WEBPMUX=OFF \
  -DWEBP_BUILD_EXTRAS=OFF -DWEBP_BUILD_LIBWEBPMUX=OFF
cmake --build "$p18_scratch/libwebp-build" --target webp -j 4
cc -O3 -Wall -Wextra -Werror -I"$p18_pinned/src" \
  "$p18_repo/tools/libwebp_vp8l_product_compare.c" \
  "$p18_scratch/libwebp-build/libwebp.a" -o "$p18_scratch/libwebp-compare"
cc -O3 -Wall -Wextra -Werror -I"$p18_pinned/src" \
  "$p18_repo/tools/libwebp_vp8l_product_bench.c" \
  "$p18_scratch/libwebp-build/libwebp.a" -o "$p18_scratch/libwebp-bench"
find "$p18_generated" -mindepth 2 -type f -name '*.webp' -print0 | sort -z |
  xargs -0 "$p18_scratch/libwebp-compare" "$p18_generated/expected" \
  > "$p18_output/raw/screen-41-correctness-final/libwebp-compare.tsv" \
  2> "$p18_output/raw/screen-41-correctness-final/libwebp-compare.stderr"

python3 "$p18_repo/tools/run-vp8l-product-benchmark.py" \
  --binary "$p18_binary" --input "$p18_screen" --generated "$p18_generated" \
  --output "$p18_output/raw/screen-41-rust-decode-final" --rounds 3 \
  --layouts "$p18_layouts" --operations decode
python3 "$p18_repo/tools/run-vp8l-libwebp-product-benchmark.py" \
  --binary "$p18_scratch/libwebp-bench" --generated "$p18_generated" \
  --expected "$p18_generated/expected" \
  --output "$p18_output/raw/screen-41-libwebp-decode-final" --rounds 3 \
  --layouts "$p18_layouts"

python3 "$p18_repo/tools/run-vp8l-product-benchmark.py" \
  --binary "$p18_binary" --input "$p18_corpus" --generated "$p18_generated" \
  --output "$p18_output/raw/formal-102x5" --rounds 5 \
  --layouts "$p18_layouts" --operations encode --formal

p18_archive="$p18_scratch/archive-candidate"
mkdir -p "$p18_output/raw/final-correctness-102"
VP8L_PRODUCT_COMMAND=generate VP8L_PRODUCT_INPUT="$p18_corpus" \
VP8L_PRODUCT_OUTPUT="$p18_archive" "$p18_binary" --exact "$p18_test" \
  --ignored --nocapture \
  > "$p18_output/raw/final-correctness-102/candidate-project-generate.tsv" \
  2> "$p18_output/raw/final-correctness-102/candidate-project-generate.stderr"
find "$p18_archive" -mindepth 2 -type f -name '*.webp' -print0 | sort -z |
  xargs -0 "$p18_scratch/libwebp-compare" "$p18_archive/expected" \
  > "$p18_output/raw/final-correctness-102/candidate-libwebp-compare.tsv" \
  2> "$p18_output/raw/final-correctness-102/candidate-libwebp-compare.stderr"

p18_control_target="$p18_scratch/control-target"
CARGO_TARGET_DIR="$p18_control_target" cargo test \
  --manifest-path "$p18_repo/webp-rs/Cargo.toml" --release -p webp --lib --no-run
p18_control_binary=$(find "$p18_control_target/release/deps" -type f -perm -111 -name 'webp-*' -print)
test "$(printf '%s\n' "$p18_control_binary" | sed '/^$/d' | wc -l | tr -d ' ')" -eq 1
p18_control="$p18_scratch/archive-control"
VP8L_PRODUCT_COMMAND=generate VP8L_PRODUCT_INPUT="$p18_corpus" \
VP8L_PRODUCT_OUTPUT="$p18_control" "$p18_control_binary" --exact "$p18_test" \
  --ignored --nocapture \
  > "$p18_output/raw/final-correctness-102/control-project-generate.tsv" \
  2> "$p18_output/raw/final-correctness-102/control-project-generate.stderr"
printf 'id\tcandidate_sha256\tcontrol_sha256\tbyte_identical\n' \
  > "$p18_output/raw/final-correctness-102/default-identity-102.tsv"
for p18_candidate in "$p18_archive"/default/*.webp; do
  p18_name=$(basename "$p18_candidate")
  p18_control_stream="$p18_control/default/$p18_name"
  p18_candidate_sha=$(shasum -a 256 "$p18_candidate" | cut -d ' ' -f 1)
  p18_control_sha=$(shasum -a 256 "$p18_control_stream" | cut -d ' ' -f 1)
  cmp -s "$p18_candidate" "$p18_control_stream"
  printf '%s\t%s\t%s\t1\n' "${p18_name%.webp}" "$p18_candidate_sha" "$p18_control_sha" \
    >> "$p18_output/raw/final-correctness-102/default-identity-102.tsv"
done

mkdir "$p18_output/raw/validation-final"
p18_validation_target="$p18_scratch/validation-target"
CARGO_TARGET_DIR="$p18_validation_target" cargo test --manifest-path "$p18_repo/webp-rs/Cargo.toml" \
  --workspace --all-targets > "$p18_output/raw/validation-final/01-default-workspace-tests.log" 2>&1
CARGO_TARGET_DIR="$p18_validation_target" cargo test --manifest-path "$p18_repo/webp-rs/Cargo.toml" \
  --workspace --all-targets --features vp8l-profile-hybrid-experiment \
  > "$p18_output/raw/validation-final/02-feature-workspace-tests.log" 2>&1
CARGO_TARGET_DIR="$p18_validation_target" cargo clippy --manifest-path "$p18_repo/webp-rs/Cargo.toml" \
  --workspace --all-targets -- -D warnings \
  > "$p18_output/raw/validation-final/03-default-clippy.log" 2>&1
CARGO_TARGET_DIR="$p18_validation_target" cargo clippy --manifest-path "$p18_repo/webp-rs/Cargo.toml" \
  --workspace --all-targets --features vp8l-profile-hybrid-experiment -- -D warnings \
  > "$p18_output/raw/validation-final/04-feature-clippy.log" 2>&1
cargo fmt --manifest-path "$p18_repo/webp-rs/Cargo.toml" --all -- --check \
  > "$p18_output/raw/validation-final/05-fmt.log" 2>&1
RUSTDOCFLAGS='-D warnings' CARGO_TARGET_DIR="$p18_validation_target" cargo doc \
  --manifest-path "$p18_repo/webp-rs/Cargo.toml" --workspace --no-deps \
  > "$p18_output/raw/validation-final/06-default-rustdoc.log" 2>&1
RUSTDOCFLAGS='-D warnings' CARGO_TARGET_DIR="$p18_validation_target" cargo doc \
  --manifest-path "$p18_repo/webp-rs/Cargo.toml" --workspace --no-deps \
  --features vp8l-profile-hybrid-experiment \
  > "$p18_output/raw/validation-final/07-feature-rustdoc.log" 2>&1
CARGO_TARGET_DIR="$p18_validation_target" cargo test --manifest-path "$p18_repo/webp-rs/Cargo.toml" \
  --workspace --doc > "$p18_output/raw/validation-final/08-default-doctest.log" 2>&1
CARGO_TARGET_DIR="$p18_validation_target" cargo test --manifest-path "$p18_repo/webp-rs/Cargo.toml" \
  --workspace --doc --features vp8l-profile-hybrid-experiment \
  > "$p18_output/raw/validation-final/09-feature-doctest.log" 2>&1

cp "$p18_repo/experiments/vp8l-profile-hybrid-clustering/summarize.py" "$p18_output/summarize.py"
python3 "$p18_output/summarize.py"
python3 - "$p18_output" <<'PY'
import csv, json, pathlib, sys
root = pathlib.Path(sys.argv[1])
phase = json.loads((root / "phase-a-summary.json").read_text())
screen = json.loads((root / "screen-summary.json").read_text())
formal = json.loads((root / "formal-summary.json").read_text())
assert phase["gate"]["phase_a_pass"]
assert phase["binary_sha256"] == screen["binary_sha256"] == formal["binary_sha256"]
assert screen["screen_gate"] and screen["correctness"]["project_exact"] == 246
assert screen["correctness"]["pinned_c_summary"] == "oracle_summary\tmatched=246\tfailed=0"
assert formal["formal_gate"]
candidate = root / "raw/final-correctness-102/candidate-project-generate.tsv"
rows = [r for r in csv.reader(candidate.open(), delimiter="\t") if r and r[0] == "stream" and r[1] != "id"]
assert len(rows) == 612 and all(r[-1] == "1" for r in rows)
oracle = (root / "raw/final-correctness-102/candidate-libwebp-compare.tsv").read_text()
assert "oracle_summary\tmatched=612\tfailed=0" in oracle
identity = list(csv.DictReader((root / "raw/final-correctness-102/default-identity-102.tsv").open(), delimiter="\t"))
assert len(identity) == 102 and all(r["byte_identical"] == "1" for r in identity)
PY
printf 'binary_sha256=%s\nphase_a_pass=true\nscreen_pass=true\nformal_pass=true\nfinal_quality_pass=true\ndecision=promote-research\n' \
  "$p18_binary_sha" > "$p18_output/reproduction-status.txt"
printf 'reference_binary_sha256=%s\nrebuilt_binary_sha256=%s\n' \
  "$p18_reference_binary_sha" "$p18_binary_sha" \
  > "$p18_output/reproduction-binary-provenance.txt"
(
  cd "$p18_output"
  find . -type f ! -name SHA256SUMS -print0 | sort -z |
    xargs -0 shasum -a 256 > SHA256SUMS
  shasum -a 256 -c SHA256SUMS
)
