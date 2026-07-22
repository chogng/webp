#!/usr/bin/env bash
set -euo pipefail

p17_repo=$(cd "$(dirname "$0")/../.." && pwd)
p17_corpus=${1:-/Users/lance/Desktop/webp/third_party/benchdata/clic/vp8l-lossless-exact}
p17_output=${2:-/private/tmp/vp8l-multires-spatial-portfolio-reproduction}
p17_base=ec7fbaf69f423bfd7251a121d2e629cfa776cb79
p17_implementation=bdb709ea9b94996971dffec3d13e5320da241434
p17_manifest_sha=9feb09f469753c43864011aa6f00cfc5ee1bd48da5aac8f9b16d105890e14f86
p17_audit_binary_sha=42ec743c6791319a8186882380b9865eccc8acb9ff00993f999842d42cb75629

if [[ -e "$p17_output" ]]; then
  echo "refusing existing output: $p17_output" >&2
  exit 2
fi
git -C "$p17_repo" merge-base --is-ancestor "$p17_implementation" HEAD
mkdir -p "$p17_output/raw/phase-a-102"
p17_scratch=$(mktemp -d /private/tmp/p17-reproduce.XXXXXX)
p17_cleanup() {
  python3 -c 'import shutil,sys; shutil.rmtree(sys.argv[1], ignore_errors=True)' "$p17_scratch"
}
trap p17_cleanup EXIT INT TERM HUP

for p17_spec in "base:$p17_base" "candidate:HEAD"; do
  p17_name=${p17_spec%%:*}
  p17_sha=${p17_spec#*:}
  mkdir "$p17_scratch/$p17_name"
  git -C "$p17_repo" archive "$p17_sha" | tar -x -C "$p17_scratch/$p17_name"
  CARGO_TARGET_DIR="$p17_scratch/$p17_name-target" \
    cargo build --manifest-path "$p17_scratch/$p17_name/webp-rs/Cargo.toml" \
    --release -p webp > "$p17_output/raw/$p17_name-build-release.log" 2>&1
  CARGO_TARGET_DIR="$p17_scratch/$p17_name-target" \
    cargo test --manifest-path "$p17_scratch/$p17_name/webp-rs/Cargo.toml" \
    --release -p webp --lib --no-run > "$p17_output/raw/$p17_name-test-no-run.log" 2>&1
done

p17_binary() {
  local p17_name=$1
  local p17_found
  local p17_count
  p17_found=$(find "$p17_scratch/$p17_name-target/release/deps" \
    -type f -perm -111 -name 'webp-*' -print | sort)
  p17_count=$(printf '%s\n' "$p17_found" | sed '/^$/d' | wc -l | tr -d ' ')
  test "$p17_count" -eq 1
  printf '%s\n' "$p17_found"
}
p17_base_binary=$(p17_binary base)
p17_candidate_binary=$(p17_binary candidate)
p17_candidate_binary_sha=$(shasum -a 256 "$p17_candidate_binary" | cut -d ' ' -f 1)
p17_base_rlib=$(find "$p17_scratch/base-target/release/deps" -type f -name 'libwebp-*.rlib' -print | head -n 1)
p17_candidate_rlib=$(find "$p17_scratch/candidate-target/release/deps" -type f -name 'libwebp-*.rlib' -print | head -n 1)
printf 'name\tcommit\ttest_binary_bytes\ttest_binary_sha256\trelease_rlib_bytes\trelease_rlib_sha256\n' > "$p17_output/raw/binary-artifacts.tsv"
printf 'base\t%s\t%s\t%s\t%s\t%s\n' "$p17_base" \
  "$(stat -f '%z' "$p17_base_binary")" "$(shasum -a 256 "$p17_base_binary" | cut -d ' ' -f 1)" \
  "$(stat -f '%z' "$p17_base_rlib")" "$(shasum -a 256 "$p17_base_rlib" | cut -d ' ' -f 1)" \
  >> "$p17_output/raw/binary-artifacts.tsv"
printf 'candidate\t%s\t%s\t%s\t%s\t%s\n' "$(git -C "$p17_repo" rev-parse HEAD)" \
  "$(stat -f '%z' "$p17_candidate_binary")" "$p17_candidate_binary_sha" \
  "$(stat -f '%z' "$p17_candidate_rlib")" "$(shasum -a 256 "$p17_candidate_rlib" | cut -d ' ' -f 1)" \
  >> "$p17_output/raw/binary-artifacts.tsv"
printf 'context\tsha256\nlocked_phase_a_worktree\t%s\nisolated_reproduction\t%s\n' \
  "$p17_audit_binary_sha" "$p17_candidate_binary_sha" \
  > "$p17_output/raw/binary-context.tsv"

find "$p17_corpus" -maxdepth 1 -type f -name '*-m6.webp' -print | sort |
  while IFS= read -r p17_path; do
    printf '%s\t%s\t%s\n' "$(basename "$p17_path")" "$(stat -f '%z' "$p17_path")" \
      "$(shasum -a 256 "$p17_path" | cut -d ' ' -f 1)"
  done > "$p17_output/raw/corpus-manifest-102.tsv"
test "$(wc -l < "$p17_output/raw/corpus-manifest-102.tsv" | tr -d ' ')" -eq 102
test "$(shasum -a 256 "$p17_output/raw/corpus-manifest-102.tsv" | cut -d ' ' -f 1)" = "$p17_manifest_sha"

VP8L_PRODUCT_COMMAND=p17-phase-a VP8L_PRODUCT_INPUT="$p17_corpus" \
  "$p17_candidate_binary" --exact \
  vp8l::image_writer::product_benchmark_tests::product_validation_reproducer \
  --ignored --nocapture > "$p17_output/raw/phase-a-102/phase-a.tsv" \
  2> "$p17_output/raw/phase-a-102/phase-a.stderr"

printf 'Not run: Phase A LowLatency image 074 exceeded the +2%% E37 per-image rate gate.\n' \
  > "$p17_output/raw/screen-41-not-run.txt"
printf 'Not run: the prerequisite Phase A gate failed, so screen and formal were prohibited.\n' \
  > "$p17_output/raw/formal-102x5-not-run.txt"
python3 "$p17_repo/experiments/vp8l-multires-spatial-portfolio/summarize.py" "$p17_output"
python3 - "$p17_output/phase-a-summary.json" <<'PY'
import json, sys
summary = json.load(open(sys.argv[1]))
assert summary["decision"] == "reject-phase-a"
assert summary["compact"]["candidate_bytes"] == 599_398_064
assert summary["low_latency"]["candidate_bytes"] == 599_169_200
assert summary["low_latency"]["over_2pct_ids"] == ["clic-validation-074"]
assert summary["exactness"]["resolution_selector_exact"] == 102
assert summary["exactness"]["public_compact_exact"] == 102
assert summary["exactness"]["public_low_exact"] == 102
PY
printf 'exit_status=0\ndecision=reject-phase-a\nscreen_41_run=false\nformal_102x5_run=false\n' \
  > "$p17_output/reproduction-status.txt"
(
  cd "$p17_output"
  find . -type f ! -name SHA256SUMS -print0 | sort -z |
    xargs -0 shasum -a 256 > SHA256SUMS
  shasum -a 256 -c SHA256SUMS
)
