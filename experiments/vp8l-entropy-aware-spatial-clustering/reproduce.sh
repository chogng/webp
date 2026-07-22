#!/usr/bin/env bash
set -euo pipefail

p15_repo=$(cd "$(dirname "$0")/../.." && pwd)
p15_corpus=${1:-/Users/lance/Desktop/webp/third_party/benchdata/clic/vp8l-lossless-exact}
p15_oracle=${2:-/Users/lance/Desktop/webp/third_party/oracle/libwebp}
p15_output=${3:-/private/tmp/vp8l-entropy-aware-spatial-clustering-reproduction}
p15_base=0e91e379aef2cfac1189472a3dd0627060f892b8
p15_control=b3b96fdc27d2076b020b6d344f196e3ffc4cc6e1
p15_candidate=7d14b83519abf3862c392f628f8f3f08e10c2556
p15_manifest_sha=9feb09f469753c43864011aa6f00cfc5ee1bd48da5aac8f9b16d105890e14f86
p15_screen_sha=474587feabe3178268b2eab6f7a166501d8ecc3d637a76bd412f4233dfa7b913
p15_layouts=compact-control,compact,low-latency-control,low-latency

if [[ -e "$p15_output" ]]; then
  echo "refusing existing output: $p15_output" >&2
  exit 2
fi
test "$(git -C "$p15_oracle" rev-parse HEAD)" = 733c91e461c18cf1127c9ed0a80dccbcfed599d3
test -f "$p15_oracle/build/libwebp.a"
for p15_sha in "$p15_base" "$p15_control" "$p15_candidate"; do
  git -C "$p15_repo" cat-file -e "$p15_sha^{commit}"
done

mkdir -p "$p15_output/raw"
p15_scratch=$(mktemp -d /private/tmp/p15-reproduce.XXXXXX)
p15_cleanup() {
  rm -f /private/tmp/webp-vp8l-product-benchmark.lock
  rm -f /private/tmp/webp-vp8l-libwebp-product-benchmark.lock
  python3 -c 'import shutil,sys; shutil.rmtree(sys.argv[1], ignore_errors=True)' "$p15_scratch"
}
trap p15_cleanup EXIT INT TERM HUP

for p15_spec in "base:$p15_base" "control:$p15_control" "candidate:$p15_candidate"; do
  p15_name=${p15_spec%%:*}
  p15_sha=${p15_spec#*:}
  mkdir "$p15_scratch/$p15_name"
  git -C "$p15_repo" archive "$p15_sha" | tar -x -C "$p15_scratch/$p15_name"
  (
    cd "$p15_scratch/$p15_name/webp-rs"
    CARGO_TARGET_DIR="$p15_scratch/$p15_name-target" cargo test --release -p webp --lib --no-run
    CARGO_TARGET_DIR="$p15_scratch/$p15_name-target" cargo build --release -p webp
  )
done

p15_binary() {
  local p15_name=$1
  local p15_found
  local p15_count
  p15_found=$(find "$p15_scratch/$p15_name-target/release/deps" -type f -perm -111 -name 'webp-*' -print | sort)
  p15_count=$(printf '%s\n' "$p15_found" | sed '/^$/d' | wc -l | tr -d ' ')
  test "$p15_count" -eq 1
  printf '%s\n' "$p15_found"
}
p15_base_binary=$(p15_binary base)
p15_control_binary=$(p15_binary control)
p15_candidate_binary=$(p15_binary candidate)
p15_candidate_binary_sha=$(shasum -a 256 "$p15_candidate_binary" | cut -d ' ' -f 1)

printf 'name\tcommit\ttest_binary_bytes\ttest_binary_sha256\trelease_rlib_bytes\trelease_rlib_sha256\n' > "$p15_output/raw/binary-artifacts.tsv"
for p15_name in base control candidate; do
  eval "p15_sha=\$p15_$p15_name"
  eval "p15_test_binary=\$p15_${p15_name}_binary"
  p15_rlib=$(find "$p15_scratch/$p15_name-target/release/deps" -type f -name 'libwebp-*.rlib' -print | head -n 1)
  printf '%s\t%s\t%s\t%s\t%s\t%s\n' "$p15_name" "$p15_sha" \
    "$(stat -f '%z' "$p15_test_binary")" "$(shasum -a 256 "$p15_test_binary" | cut -d ' ' -f 1)" \
    "$(stat -f '%z' "$p15_rlib")" "$(shasum -a 256 "$p15_rlib" | cut -d ' ' -f 1)" \
    >> "$p15_output/raw/binary-artifacts.tsv"
done

p15_manifest="$p15_output/raw/corpus-manifest-102.tsv"
find "$p15_corpus" -maxdepth 1 -type f -name '*-m6.webp' -print | sort |
  while IFS= read -r p15_path; do
    printf '%s\t%s\t%s\n' "$(basename "$p15_path")" "$(stat -f '%z' "$p15_path")" \
      "$(shasum -a 256 "$p15_path" | cut -d ' ' -f 1)"
  done > "$p15_manifest"
test "$(wc -l < "$p15_manifest" | tr -d ' ')" -eq 102
test "$(shasum -a 256 "$p15_manifest" | cut -d ' ' -f 1)" = "$p15_manifest_sha"
head -n 41 "$p15_manifest" > "$p15_output/raw/screen-manifest-41.tsv"
test "$(shasum -a 256 "$p15_output/raw/screen-manifest-41.tsv" | cut -d ' ' -f 1)" = "$p15_screen_sha"
p15_screen="$p15_scratch/screen"
mkdir "$p15_screen"
while IFS=$'\t' read -r p15_name _; do
  ln -s "$p15_corpus/$p15_name" "$p15_screen/$p15_name"
done < "$p15_output/raw/screen-manifest-41.tsv"

mkdir -p "$p15_output/raw/phase-a-102"
set -C
: > /private/tmp/webp-vp8l-product-benchmark.lock
VP8L_PRODUCT_COMMAND=p15-phase-a VP8L_PRODUCT_INPUT="$p15_corpus" \
  "$p15_candidate_binary" --exact encoder::product_benchmark_tests::product_validation_reproducer \
  --ignored --nocapture > "$p15_output/raw/phase-a-102/phase-a.tsv" \
  2> "$p15_output/raw/phase-a-102/phase-a.stderr"
rm -f /private/tmp/webp-vp8l-product-benchmark.lock
set +C
printf '{"binary_sha256":"%s"}\n' "$p15_candidate_binary_sha" > "$p15_output/phase-a-summary.json"

python3 "$p15_repo/tools/run-vp8l-product-benchmark.py" \
  --binary "$p15_candidate_binary" --input "$p15_screen" --generated "$p15_scratch/unused" \
  --output "$p15_output/raw/screen-41-encode" --rounds 3 \
  --layouts "$p15_layouts" --operations encode

p15_generated="$p15_scratch/generated"
mkdir -p "$p15_output/raw/screen-41-correctness"
VP8L_PRODUCT_COMMAND=generate VP8L_PRODUCT_INPUT="$p15_screen" \
VP8L_PRODUCT_OUTPUT="$p15_generated" "$p15_candidate_binary" --exact \
  encoder::product_benchmark_tests::product_validation_reproducer --ignored --nocapture \
  > "$p15_output/raw/screen-41-correctness/project-generate.tsv" \
  2> "$p15_output/raw/screen-41-correctness/project-generate.stderr"

cc -O3 -Wall -Wextra -Werror -I"$p15_oracle/src" \
  "$p15_repo/tools/libwebp_vp8l_product_compare.c" "$p15_oracle/build/libwebp.a" \
  -o "$p15_scratch/libwebp-compare"
cc -O3 -Wall -Wextra -Werror -I"$p15_oracle/src" \
  "$p15_repo/tools/libwebp_vp8l_product_bench.c" "$p15_oracle/build/libwebp.a" \
  -o "$p15_scratch/libwebp-bench"
find "$p15_generated" -mindepth 2 -type f -name '*.webp' -print | sort > "$p15_scratch/streams.txt"
"$p15_scratch/libwebp-compare" "$p15_generated/expected" $(< "$p15_scratch/streams.txt") \
  > "$p15_output/raw/screen-41-correctness/libwebp-compare.tsv" \
  2> "$p15_output/raw/screen-41-correctness/libwebp-compare.stderr"

python3 "$p15_repo/tools/run-vp8l-product-benchmark.py" \
  --binary "$p15_candidate_binary" --input "$p15_screen" --generated "$p15_generated" \
  --output "$p15_output/raw/screen-41-rust-decode" --rounds 3 \
  --layouts "$p15_layouts" --operations decode
python3 "$p15_repo/tools/run-vp8l-libwebp-product-benchmark.py" \
  --binary "$p15_scratch/libwebp-bench" --generated "$p15_generated" \
  --expected "$p15_generated/expected" \
  --output "$p15_output/raw/screen-41-libwebp-decode" --rounds 3 \
  --layouts "$p15_layouts"

python3 "$p15_repo/experiments/vp8l-entropy-aware-spatial-clustering/verify_identity.py" \
  --base-binary "$p15_base_binary" --base-label creation-base-0e91e379 \
  --control-binary "$p15_control_binary" --control-label e37-b3b96fdc \
  --candidate-binary "$p15_candidate_binary" --candidate-label p15-7d14b835 \
  --corpus "$p15_corpus" --oracle-binary "$p15_scratch/libwebp-compare" \
  --output "$p15_output/raw/identity-306-final"

p15_validation="$p15_output/raw/validation-final"
mkdir "$p15_validation"
printf 'name\tstatus\tlog\n' > "$p15_validation/validation.tsv"
p15_run() {
  local p15_name=$1
  shift
  set +e
  (cd "$p15_scratch/candidate" && "$@") > "$p15_validation/$p15_name.log" 2>&1
  local p15_status=$?
  set -e
  printf '%s\t%s\t%s\n' "$p15_name" "$p15_status" "$p15_name.log" >> "$p15_validation/validation.tsv"
  return "$p15_status"
}
p15_run test-workspace env CARGO_TARGET_DIR="$p15_scratch/validation-target" cargo test --manifest-path webp-rs/Cargo.toml --workspace --all-targets
p15_run test-release env CARGO_TARGET_DIR="$p15_scratch/validation-target" cargo test --manifest-path webp-rs/Cargo.toml --release --workspace --all-targets
p15_run build-release env CARGO_TARGET_DIR="$p15_scratch/validation-target" cargo build --manifest-path webp-rs/Cargo.toml --release --workspace --all-targets
p15_run clippy env CARGO_TARGET_DIR="$p15_scratch/validation-target" cargo clippy --manifest-path webp-rs/Cargo.toml --workspace --all-targets -- -D warnings
p15_run fmt cargo fmt --manifest-path webp-rs/Cargo.toml --all -- --check
p15_run rustdoc env 'RUSTDOCFLAGS=-D warnings' CARGO_TARGET_DIR="$p15_scratch/validation-target" cargo doc --manifest-path webp-rs/Cargo.toml -p webp --no-deps
p15_run doctest env CARGO_TARGET_DIR="$p15_scratch/validation-target" cargo test --manifest-path webp-rs/Cargo.toml -p webp --doc
rustc --version --verbose > "$p15_validation/toolchain.txt"
rustup target list --installed > "$p15_validation/installed-targets.txt"

printf 'Formal 102x5 was not run: LowLatency image 008 exceeds the +2%% screen rate limit.\n' > "$p15_output/raw/formal-102x5-not-run.txt"
python3 "$p15_repo/experiments/vp8l-entropy-aware-spatial-clustering/summarize.py" "$p15_output"
python3 - "$p15_output/gate-summary.json" <<'PY'
import json, sys
s = json.load(open(sys.argv[1]))
assert s["decision"] == "reject-screen"
assert not s["formal_102x5_run"]
assert s["phase_a"]["compact"]["final_bytes"] == 599_398_064
assert s["phase_a"]["low_latency"]["final_bytes"] == 617_047_520
assert s["screen"]["low_latency"]["encode"]["max_image_rate_id"] == "clic-validation-008"
assert s["screen"]["low_latency"]["encode"]["images_rate_over_2pct"] == 1
assert s["correctness"]["archive_project_exact"] == 918
assert s["correctness"]["archive_pinned_c_exact"] == 918
assert s["quality"]["passed"] == s["quality"]["total"] == 7
PY

printf 'exit_status=0\nformal_102x5_run=false\ndecision=reject-screen\n' > "$p15_output/reproduction-status.txt"
(
  cd "$p15_output"
  find . -type f ! -name SHA256SUMS -print0 | sort -z | xargs -0 shasum -a 256 > SHA256SUMS
  shasum -a 256 -c SHA256SUMS
)
