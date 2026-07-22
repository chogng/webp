#!/usr/bin/env bash
set -euo pipefail

p13_repo=$(cd "$(dirname "$0")/../.." && pwd)
p13_corpus=${1:-/Users/lance/Desktop/webp/third_party/benchdata/clic/vp8l-lossless-exact}
p13_oracle=${2:-/Users/lance/Desktop/webp/third_party/oracle/libwebp}
p13_output=${3:-/private/tmp/vp8l-streaming-spatial-plan-reproduction}
p13_base_sha=cec68762e5ab6184bce275aeff5720ba3e6f96c7
p13_control_sha=b3b96fdc27d2076b020b6d344f196e3ffc4cc6e1
p13_initial_sha=f5e5bee5ba4a455da31678f90d89ac6d15368bae
p13_fixed_sha=815df5465b4266bd6e5cf1adbd3dcbb8e3b8c20c
p13_candidate_sha=292c1d74cbc024207bf91c4a40d720c36190f0e2
p13_manifest_sha=9feb09f469753c43864011aa6f00cfc5ee1bd48da5aac8f9b16d105890e14f86
p13_screen_manifest_sha=474587feabe3178268b2eab6f7a166501d8ecc3d637a76bd412f4233dfa7b913

if [[ -e "$p13_output" ]]; then
  echo "refusing existing output: $p13_output" >&2
  exit 2
fi
for p13_sha in "$p13_base_sha" "$p13_control_sha" "$p13_initial_sha" \
  "$p13_fixed_sha" "$p13_candidate_sha"; do
  git -C "$p13_repo" cat-file -e "$p13_sha^{commit}"
done
test "$(git -C "$p13_oracle" rev-parse HEAD)" = 733c91e461c18cf1127c9ed0a80dccbcfed599d3
test -f "$p13_oracle/build/libwebp.a"

mkdir -p "$p13_output/raw"
p13_scratch=$(mktemp -d /private/tmp/vp8l-streaming-spatial-plan.XXXXXX)
p13_cleanup() {
  python3 -c 'import shutil,sys; shutil.rmtree(sys.argv[1], ignore_errors=True)' "$p13_scratch"
}
trap p13_cleanup EXIT INT TERM HUP

printf 'name\tcommit\nbase\t%s\ncontrol\t%s\ninitial\t%s\nfixed\t%s\ncandidate\t%s\n' \
  "$p13_base_sha" "$p13_control_sha" "$p13_initial_sha" "$p13_fixed_sha" \
  "$p13_candidate_sha" \
  > "$p13_output/archive-commits.tsv"
for p13_spec in "base:$p13_base_sha" "control:$p13_control_sha" \
  "initial:$p13_initial_sha" "fixed:$p13_fixed_sha" "candidate:$p13_candidate_sha"; do
  p13_name=${p13_spec%%:*}
  p13_sha=${p13_spec#*:}
  mkdir "$p13_scratch/$p13_name"
  git -C "$p13_repo" archive "$p13_sha" | tar -x -C "$p13_scratch/$p13_name"
  test -f "$p13_scratch/$p13_name/AGENTS.md"
  test -f "$p13_scratch/$p13_name/webp-rs/Cargo.toml"
  (
    cd "$p13_scratch/$p13_name/webp-rs"
    CARGO_TARGET_DIR="$p13_scratch/$p13_name-target" cargo test --release -p webp --lib --no-run
    CARGO_TARGET_DIR="$p13_scratch/$p13_name-target" cargo build --release -p webp
  )
done

p13_find_binary() {
  local p13_name=$1
  local p13_found
  local p13_count
  p13_found=$(find "$p13_scratch/$p13_name-target/release/deps" \
    -type f -perm -111 -name 'webp-*' -print | sort)
  p13_count=$(printf '%s\n' "$p13_found" | sed '/^$/d' | wc -l | tr -d ' ')
  if [[ $p13_count -ne 1 ]]; then
    echo "$p13_name: expected one release test binary, found $p13_count" >&2
    return 2
  fi
  printf '%s\n' "$p13_found"
}
p13_base_binary=$(p13_find_binary base)
p13_control_binary=$(p13_find_binary control)
p13_initial_binary=$(p13_find_binary initial)
p13_fixed_binary=$(p13_find_binary fixed)
p13_candidate_binary=$(p13_find_binary candidate)

printf 'name\tcommit\ttest_binary_bytes\ttest_binary_sha256\trelease_rlib_bytes\trelease_rlib_sha256\n' \
  > "$p13_output/binary-artifacts.tsv"
for p13_name in base control candidate; do
  case "$p13_name" in
    base) p13_sha=$p13_base_sha; p13_binary=$p13_base_binary ;;
    control) p13_sha=$p13_control_sha; p13_binary=$p13_control_binary ;;
    candidate) p13_sha=$p13_candidate_sha; p13_binary=$p13_candidate_binary ;;
  esac
  p13_rlib=$(find "$p13_scratch/$p13_name-target/release/deps" -type f -name 'libwebp-*.rlib' -print | sort | head -n 1)
  printf '%s\t%s\t%s\t%s\t%s\t%s\n' \
    "$p13_name" "$p13_sha" "$(stat -f '%z' "$p13_binary")" \
    "$(shasum -a 256 "$p13_binary" | cut -d ' ' -f 1)" \
    "$(stat -f '%z' "$p13_rlib")" "$(shasum -a 256 "$p13_rlib" | cut -d ' ' -f 1)" \
    >> "$p13_output/binary-artifacts.tsv"
done

p13_manifest="$p13_output/raw/corpus-manifest-102.tsv"
find "$p13_corpus" -maxdepth 1 -type f -name '*-m6.webp' -print | sort |
  while IFS= read -r p13_path; do
    printf '%s\t%s\t%s\n' "$(basename "$p13_path")" "$(stat -f '%z' "$p13_path")" \
      "$(shasum -a 256 "$p13_path" | cut -d ' ' -f 1)"
  done > "$p13_manifest"
test "$(wc -l < "$p13_manifest")" -eq 102
test "$(shasum -a 256 "$p13_manifest" | cut -d ' ' -f 1)" = "$p13_manifest_sha"
cmp "$p13_manifest" \
  "$p13_repo/experiments/vp8l-packed-writer-product/raw/corpus-manifest-102.tsv"

p13_screen_input="$p13_scratch/screen-input"
mkdir "$p13_screen_input"
while IFS=$'\t' read -r p13_name _; do
  ln -s "$p13_corpus/$p13_name" "$p13_screen_input/$p13_name"
done < <(head -n 41 "$p13_manifest")
head -n 41 "$p13_manifest" > "$p13_output/raw/screen-manifest-41.tsv"
test "$(shasum -a 256 "$p13_output/raw/screen-manifest-41.tsv" | cut -d ' ' -f 1)" = \
  "$p13_screen_manifest_sha"
cmp "$p13_output/raw/screen-manifest-41.tsv" \
  "$p13_repo/experiments/vp8l-packed-writer-product/raw/screen-manifest-41.tsv"

VP8L_PRODUCT_COMMAND=audit-streaming-spatial-phases VP8L_PRODUCT_INPUT="$p13_corpus" \
  "$p13_candidate_binary" --exact \
  encoder::product_benchmark_tests::product_validation_reproducer --ignored --nocapture \
  > "$p13_output/raw/phase-a-102.tsv" 2> "$p13_output/raw/phase-a-102.stderr"

p13_variant_layouts=compact-pipeline-control,compact-streaming,compact-streaming-census,compact-streaming-census-frequencies,low-latency-pipeline-control,low-latency-streaming,low-latency-streaming-census,low-latency-streaming-census-frequencies
python3 "$p13_repo/tools/run-vp8l-product-benchmark.py" \
  --binary "$p13_initial_binary" --input "$p13_screen_input" \
  --generated "$p13_scratch/unused" \
  --output "$p13_output/raw/screen-41-variants-f5e5bee5" \
  --rounds 3 --layouts "$p13_variant_layouts" --operations encode
python3 "$p13_repo/tools/run-vp8l-product-benchmark.py" \
  --binary "$p13_fixed_binary" --input "$p13_screen_input" \
  --generated "$p13_scratch/unused" \
  --output "$p13_output/raw/screen-41-variants-815df546" \
  --rounds 3 --layouts "$p13_variant_layouts" --operations encode
p13_diagnostic_layouts=compact-pipeline-control,compact-materialized-census-frequencies,low-latency-pipeline-control,low-latency-materialized-census-frequencies
python3 "$p13_repo/tools/run-vp8l-product-benchmark.py" \
  --binary "$p13_candidate_binary" --input "$p13_screen_input" \
  --generated "$p13_scratch/unused" \
  --output "$p13_output/raw/screen-41-materialized-cf-292c1d74" \
  --rounds 3 --layouts "$p13_diagnostic_layouts" --operations encode

mkdir "$p13_output/raw/phase-a-102"
mv "$p13_output/raw/phase-a-102.tsv" "$p13_output/raw/phase-a-102/phase-a.tsv"
mv "$p13_output/raw/phase-a-102.stderr" "$p13_output/raw/phase-a-102/phase-a.stderr"
python3 "$p13_repo/experiments/vp8l-streaming-spatial-plan/summarize.py" "$p13_output" \
  || exit 1
python3 - "$p13_output/gate-summary.json" <<'PY'
import json, sys
s = json.load(open(sys.argv[1]))
assert s["decision"] == "reject"
assert not any(
    row["screen_gate"]
    for stage in s["variant_screens"].values()
    for profile in stage.values()
    for row in profile.values()
)
assert not any(row["screen_gate"] for row in s["materialized_cf_diagnostic"].values())
PY

cc -O3 -Wall -Wextra -Werror -I"$p13_oracle/src" \
  "$p13_repo/tools/libwebp_vp8l_product_compare.c" "$p13_oracle/build/libwebp.a" \
  -o "$p13_scratch/libwebp-compare"
p13_identity_status=0
python3 "$p13_repo/experiments/vp8l-streaming-spatial-plan/verify_identity.py" \
  --base-binary "$p13_base_binary" --base-label "base-${p13_base_sha:0:8}" \
  --control-binary "$p13_control_binary" --control-label "e37-${p13_control_sha:0:8}" \
  --candidate-binary "$p13_candidate_binary" --candidate-label "p13-${p13_candidate_sha:0:8}" \
  --corpus "$p13_corpus" --oracle-binary "$p13_scratch/libwebp-compare" \
  --output "$p13_output/raw/identity-306" || p13_identity_status=$?

p13_validation="$p13_output/raw/validation"
mkdir "$p13_validation"
printf 'name\tstatus\tlog\n' > "$p13_validation/validation.tsv"
p13_validation_failed=0
p13_run_validation() {
  local p13_name=$1
  shift
  set +e
  (cd "$p13_scratch/candidate/webp-rs" && "$@") \
    > "$p13_validation/$p13_name.log" 2>&1
  local p13_status=$?
  set -e
  printf '%s\t%s\t%s\n' "$p13_name" "$p13_status" "$p13_name.log" \
    >> "$p13_validation/validation.tsv"
  if [[ $p13_status -ne 0 ]]; then
    p13_validation_failed=1
  fi
}
p13_run_validation test-debug env CARGO_TARGET_DIR="$p13_scratch/validation-target" \
  cargo test --workspace --all-targets
p13_run_validation test-release env CARGO_TARGET_DIR="$p13_scratch/validation-target" \
  cargo test --release --workspace --all-targets
p13_run_validation build-release env CARGO_TARGET_DIR="$p13_scratch/validation-target" \
  cargo build --release --workspace --all-targets
p13_run_validation clippy env CARGO_TARGET_DIR="$p13_scratch/validation-target" \
  cargo clippy --workspace --all-targets -- -D warnings
p13_run_validation fmt cargo fmt --all -- --check
p13_run_validation rustdoc env "RUSTDOCFLAGS=-D warnings" \
  CARGO_TARGET_DIR="$p13_scratch/validation-target" cargo doc -p webp --no-deps
p13_run_validation doctest env CARGO_TARGET_DIR="$p13_scratch/validation-target" \
  cargo test -p webp --doc

p13_final_status=0
if [[ $p13_identity_status -ne 0 ]]; then
  p13_final_status=1
fi
if [[ $p13_validation_failed -ne 0 ]]; then
  p13_final_status=1
fi
(
  cd "$p13_output"
  find . -type f ! -name SHA256SUMS -print0 | sort -z |
    xargs -0 shasum -a 256 > SHA256SUMS
  shasum -a 256 -c SHA256SUMS
)
exit "$p13_final_status"
