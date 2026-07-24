#!/usr/bin/env bash
# Benchmark one Rust VP8L candidate against the recorded baselines.
set -euo pipefail

promote=false
if [[ "${1:-}" == "--promote" ]]; then
  promote=true
  shift
fi
iterations="${1:-5}"
if ! [[ "$iterations" =~ ^[1-9][0-9]*$ ]]; then
  echo "usage: $0 [--promote] [positive iterations]" >&2
  exit 2
fi
if [[ "$#" -gt 1 ]]; then
  echo "usage: $0 [--promote] [positive iterations]" >&2
  exit 2
fi

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
. "$root/tools/temporary.sh"
. "$root/tools/benchmark-vp8l-common.sh"

results_document="$root/third_party/benchmarks/libwebp/vp8l-encode.md"
if [[ ! -f "$results_document" ]]; then
  echo "no fixed libwebp reference; run tools/benchmark-vp8l-reference.sh once" >&2
  exit 1
fi

vp8l_collect_benchmark_inputs "$root"
scratch="$(webp_mktemp_dir "$root" webp-vp8l-rust-candidate)"
webp_cleanup_on_exit "$scratch"
input_manifest="$scratch/inputs.txt"
results="$scratch/rust-results.txt"
cargo_target="$scratch/cargo-target"
vp8l_write_benchmark_input_manifest "$input_manifest"

WEBP_RS_LOSSLESS_PROFILE=default CARGO_TARGET_DIR="$cargo_target" \
  cargo run --quiet --release -p webp \
  --example encode_bench --manifest-path "$root/webp-rs/Cargo.toml" \
  -- 1 "${vp8l_benchmark_inputs[@]}" >/dev/null
WEBP_RS_LOSSLESS_PROFILE=high-compression CARGO_TARGET_DIR="$cargo_target" \
  cargo run --quiet --release -p webp \
  --example encode_bench --manifest-path "$root/webp-rs/Cargo.toml" \
  -- 1 "${vp8l_benchmark_inputs[@]}" >/dev/null

: >"$results"
WEBP_RS_LOSSLESS_PROFILE=default CARGO_TARGET_DIR="$cargo_target" \
  cargo run --quiet --release -p webp \
  --example encode_bench --manifest-path "$root/webp-rs/Cargo.toml" \
  -- "$iterations" "${vp8l_benchmark_inputs[@]}" | tee -a "$results"
WEBP_RS_LOSSLESS_PROFILE=high-compression CARGO_TARGET_DIR="$cargo_target" \
  cargo run --quiet --release -p webp \
  --example encode_bench --manifest-path "$root/webp-rs/Cargo.toml" \
  -- "$iterations" "${vp8l_benchmark_inputs[@]}" | tee -a "$results"

manager_arguments=(
  candidate
  --root "$root"
  --iterations "$iterations"
  --inputs "$input_manifest"
  --results "$results"
  --document "$results_document"
)
if [[ "$promote" == true ]]; then
  manager_arguments+=(--promote)
fi
python3 "$root/tools/update-vp8l-encode-baseline.py" "${manager_arguments[@]}"
