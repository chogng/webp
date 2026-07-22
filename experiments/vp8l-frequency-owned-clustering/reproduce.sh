#!/usr/bin/env bash
set -euo pipefail

p14_repo=$(cd "$(dirname "$0")/../.." && pwd)
p14_corpus=${1:-/Users/lance/Desktop/webp/third_party/benchdata/clic/vp8l-lossless-exact}
p14_oracle=${2:-/Users/lance/Desktop/webp/third_party/oracle/libwebp}
p14_output=${3:-/private/tmp/vp8l-frequency-owned-clustering-reproduction}
p14_base=3474599d89804cb91357788e967826544903011c
p14_control=b3b96fdc27d2076b020b6d344f196e3ffc4cc6e1
p14_exact=5c5099557618310d5edd1eb45353738a7e253152
p14_coarse=2d529c33e923df722ecd37d5964e9e89d46792bf
p14_manifest_sha=9feb09f469753c43864011aa6f00cfc5ee1bd48da5aac8f9b16d105890e14f86
p14_screen_sha=474587feabe3178268b2eab6f7a166501d8ecc3d637a76bd412f4233dfa7b913

if [[ -e "$p14_output" ]]; then
  echo "refusing existing output: $p14_output" >&2
  exit 2
fi
test "$(git -C "$p14_oracle" rev-parse HEAD)" = 733c91e461c18cf1127c9ed0a80dccbcfed599d3
test -f "$p14_oracle/build/libwebp.a"
for p14_sha in "$p14_base" "$p14_control" "$p14_exact" "$p14_coarse"; do
  git -C "$p14_repo" cat-file -e "$p14_sha^{commit}"
done

mkdir -p "$p14_output/raw"
p14_scratch=$(mktemp -d /private/tmp/p14-reproduce.XXXXXX)
p14_cleanup() {
  rm -f /private/tmp/webp-vp8l-product-benchmark.lock
  rm -f /private/tmp/webp-vp8l-libwebp-product-benchmark.lock
  python3 -c 'import shutil,sys; shutil.rmtree(sys.argv[1], ignore_errors=True)' "$p14_scratch"
}
trap p14_cleanup EXIT INT TERM HUP

for p14_spec in "base:$p14_base" "control:$p14_control" "exact:$p14_exact" "coarse:$p14_coarse"; do
  p14_name=${p14_spec%%:*}
  p14_sha=${p14_spec#*:}
  mkdir "$p14_scratch/$p14_name"
  git -C "$p14_repo" archive "$p14_sha" | tar -x -C "$p14_scratch/$p14_name"
  (
    cd "$p14_scratch/$p14_name/webp-rs"
    CARGO_TARGET_DIR="$p14_scratch/$p14_name-target" cargo test --release -p webp --lib --no-run
    CARGO_TARGET_DIR="$p14_scratch/$p14_name-target" cargo build --release -p webp
  )
done

p14_binary() {
  local p14_name=$1
  local p14_found
  local p14_count
  p14_found=$(find "$p14_scratch/$p14_name-target/release/deps" -type f -perm -111 -name 'webp-*' -print | sort)
  p14_count=$(printf '%s\n' "$p14_found" | sed '/^$/d' | wc -l | tr -d ' ')
  test "$p14_count" -eq 1
  printf '%s\n' "$p14_found"
}
p14_base_binary=$(p14_binary base)
p14_control_binary=$(p14_binary control)
p14_exact_binary=$(p14_binary exact)
p14_coarse_binary=$(p14_binary coarse)

printf 'name\tcommit\ttest_binary_bytes\ttest_binary_sha256\trelease_rlib_bytes\trelease_rlib_sha256\n' > "$p14_output/raw/binary-artifacts.tsv"
for p14_name in base control exact coarse; do
  eval "p14_sha=\$p14_$p14_name"
  eval "p14_test_binary=\$p14_${p14_name}_binary"
  p14_rlib=$(find "$p14_scratch/$p14_name-target/release/deps" -type f -name 'libwebp-*.rlib' -print | head -n 1)
  printf '%s\t%s\t%s\t%s\t%s\t%s\n' "$p14_name" "$p14_sha" \
    "$(stat -f '%z' "$p14_test_binary")" "$(shasum -a 256 "$p14_test_binary" | cut -d ' ' -f 1)" \
    "$(stat -f '%z' "$p14_rlib")" "$(shasum -a 256 "$p14_rlib" | cut -d ' ' -f 1)" \
    >> "$p14_output/raw/binary-artifacts.tsv"
done

p14_manifest="$p14_output/raw/corpus-manifest-102.tsv"
find "$p14_corpus" -maxdepth 1 -type f -name '*-m6.webp' -print | sort |
  while IFS= read -r p14_path; do
    printf '%s\t%s\t%s\n' "$(basename "$p14_path")" "$(stat -f '%z' "$p14_path")" \
      "$(shasum -a 256 "$p14_path" | cut -d ' ' -f 1)"
  done > "$p14_manifest"
test "$(wc -l < "$p14_manifest")" -eq 102
test "$(shasum -a 256 "$p14_manifest" | cut -d ' ' -f 1)" = "$p14_manifest_sha"
head -n 41 "$p14_manifest" > "$p14_output/raw/screen-manifest-41.tsv"
test "$(shasum -a 256 "$p14_output/raw/screen-manifest-41.tsv" | cut -d ' ' -f 1)" = "$p14_screen_sha"
p14_screen="$p14_scratch/screen"
mkdir "$p14_screen"
while IFS=$'\t' read -r p14_name _; do
  ln -s "$p14_corpus/$p14_name" "$p14_screen/$p14_name"
done < "$p14_output/raw/screen-manifest-41.tsv"

mkdir -p "$p14_output/raw/phase-a-102"
set -C
: > /private/tmp/webp-vp8l-product-benchmark.lock
VP8L_PRODUCT_COMMAND=audit-frequency-owned VP8L_PRODUCT_INPUT="$p14_corpus" \
  "$p14_exact_binary" --exact encoder::product_benchmark_tests::product_validation_reproducer \
  --ignored --nocapture > "$p14_output/raw/phase-a-102/phase-a.tsv" \
  2> "$p14_output/raw/phase-a-102/phase-a.stderr"
rm -f /private/tmp/webp-vp8l-product-benchmark.lock
set +C

python3 "$p14_repo/tools/run-vp8l-product-benchmark.py" \
  --binary "$p14_exact_binary" --input "$p14_screen" --generated "$p14_scratch/unused" \
  --output "$p14_output/raw/screen-41-exact-symbol" --rounds 3 \
  --layouts compact-ordered-product-control,compact-frequency-owned,low-latency-ordered-product-control,low-latency-frequency-owned \
  --operations encode

p14_generated="$p14_scratch/exact-generated"
mkdir -p "$p14_output/raw/correctness-exact-symbol"
VP8L_PRODUCT_COMMAND=generate-frequency-owned VP8L_PRODUCT_INPUT="$p14_screen" \
VP8L_PRODUCT_OUTPUT="$p14_generated" "$p14_exact_binary" --exact \
  encoder::product_benchmark_tests::product_validation_reproducer --ignored --nocapture \
  > "$p14_output/raw/correctness-exact-symbol/project-generate.tsv" \
  2> "$p14_output/raw/correctness-exact-symbol/project-generate.stderr"

cc -O3 -Wall -Wextra -Werror -I"$p14_oracle/src" \
  "$p14_repo/tools/libwebp_vp8l_product_compare.c" "$p14_oracle/build/libwebp.a" \
  -o "$p14_scratch/libwebp-compare"
cc -O3 -Wall -Wextra -Werror -I"$p14_oracle/src" \
  "$p14_repo/tools/libwebp_vp8l_product_bench.c" "$p14_oracle/build/libwebp.a" \
  -o "$p14_scratch/libwebp-bench"
find "$p14_generated" -mindepth 2 -type f -name '*.webp' -print | sort > "$p14_scratch/exact-streams.txt"
"$p14_scratch/libwebp-compare" "$p14_generated/expected" $(< "$p14_scratch/exact-streams.txt") \
  > "$p14_output/raw/correctness-exact-symbol/libwebp-oracle.tsv" \
  2> "$p14_output/raw/correctness-exact-symbol/libwebp-oracle.stderr"

python3 "$p14_repo/tools/run-vp8l-product-benchmark.py" \
  --binary "$p14_exact_binary" --input "$p14_screen" --generated "$p14_generated" \
  --output "$p14_output/raw/screen-41-exact-symbol-rust-decode" --rounds 3 \
  --layouts compact-ordered-product-control,compact-frequency-owned,low-latency-ordered-product-control,low-latency-frequency-owned \
  --operations decode
python3 "$p14_repo/tools/run-vp8l-libwebp-product-benchmark.py" \
  --binary "$p14_scratch/libwebp-bench" --generated "$p14_generated" \
  --expected "$p14_generated/expected" \
  --output "$p14_output/raw/screen-41-exact-symbol-libwebp-decode" --rounds 3 \
  --layouts compact-ordered-product-control,compact-frequency-owned,low-latency-ordered-product-control,low-latency-frequency-owned

mkdir -p "$p14_output/raw/phase-b-102"
set -C
: > /private/tmp/webp-vp8l-product-benchmark.lock
VP8L_PRODUCT_COMMAND=audit-coarse-bin-mass VP8L_PRODUCT_INPUT="$p14_corpus" \
  "$p14_coarse_binary" --exact encoder::product_benchmark_tests::product_validation_reproducer \
  --ignored --nocapture > "$p14_output/raw/phase-b-102/phase-b.tsv" \
  2> "$p14_output/raw/phase-b-102/phase-b.stderr"
rm -f /private/tmp/webp-vp8l-product-benchmark.lock
set +C

python3 "$p14_repo/experiments/vp8l-frequency-owned-clustering/verify_identity.py" \
  --base-binary "$p14_base_binary" --base-label base-3474599d \
  --control-binary "$p14_control_binary" --control-label e37-b3b96fdc \
  --candidate-binary "$p14_coarse_binary" --candidate-label p14b-2d529c33 \
  --corpus "$p14_corpus" --oracle-binary "$p14_scratch/libwebp-compare" \
  --output "$p14_output/raw/identity-306-final"

p14_validation="$p14_output/raw/validation-final"
mkdir "$p14_validation"
printf 'name\tstatus\tlog\n' > "$p14_validation/validation.tsv"
p14_run() {
  local p14_name=$1
  shift
  set +e
  (cd "$p14_scratch/coarse/webp-rs" && "$@") > "$p14_validation/$p14_name.log" 2>&1
  local p14_status=$?
  set -e
  printf '%s\t%s\t%s\n' "$p14_name" "$p14_status" "$p14_name.log" >> "$p14_validation/validation.tsv"
  return "$p14_status"
}
p14_run test-workspace env CARGO_TARGET_DIR="$p14_scratch/validation-target" cargo test --workspace --all-targets
p14_run test-release env CARGO_TARGET_DIR="$p14_scratch/validation-target" cargo test --release --workspace --all-targets
p14_run build-release env CARGO_TARGET_DIR="$p14_scratch/validation-target" cargo build --release --workspace --all-targets
p14_run clippy env CARGO_TARGET_DIR="$p14_scratch/validation-target" cargo clippy --workspace --all-targets -- -D warnings
p14_run fmt cargo fmt --all -- --check
p14_run rustdoc env 'RUSTDOCFLAGS=-D warnings' CARGO_TARGET_DIR="$p14_scratch/validation-target" cargo doc -p webp --no-deps
p14_run doctest env CARGO_TARGET_DIR="$p14_scratch/validation-target" cargo test -p webp --doc
rustc --version --verbose > "$p14_validation/toolchain.txt"

python3 "$p14_repo/experiments/vp8l-frequency-owned-clustering/summarize.py" "$p14_output"
python3 - "$p14_output/gate-summary.json" <<'PY'
import json, sys
s = json.load(open(sys.argv[1]))
assert s["decision"] == "reject"
assert not s["formal_102x5_run"]
assert not s["phase_b_rate_prescreen"]
assert not any(row["encode"]["screen_gate"] for row in s["exact_symbol_screen"].values())
PY

printf 'exit_status=0\nformal_102x5_run=false\ndecision=reject\n' > "$p14_output/reproduction-status.txt"
(
  cd "$p14_output"
  find . -type f ! -name SHA256SUMS -print0 | sort -z | xargs -0 shasum -a 256 > SHA256SUMS
  shasum -a 256 -c SHA256SUMS
)
