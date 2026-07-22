#!/usr/bin/env bash
# Verify benchmark runners without downloading the pinned benchmark corpora.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
manifest="$root/webp-rs/Cargo.toml"
fixture="$root/experiments/vp8l-color-transform-fix-product/reproducer-129x129.webp"
scripts=(
  "$root/tools/benchmark-alpha-encode.sh"
  "$root/tools/benchmark-animation-encode.sh"
  "$root/tools/benchmark-demux.sh"
  "$root/tools/benchmark-mux-editor.sh"
  "$root/tools/benchmark-sharp-yuv.sh"
  "$root/tools/benchmark-vp8-encode.sh"
  "$root/tools/benchmark-vp8l-clic.sh"
  "$root/tools/benchmark-vp8l-encode.sh"
  "$root/tools/benchmark-vp8l.sh"
)

for script in "${scripts[@]}"; do
  bash -n "$script"
done

if rg -n '\$root/(webp-rs/)?target/release/examples' "${scripts[@]}"; then
  echo "benchmark scripts must let Cargo locate Rust runners" >&2
  exit 1
fi

if [[ ! -f "$fixture" ]]; then
  echo "missing benchmark smoke fixture: $fixture" >&2
  exit 1
fi

run_example() {
  local example="$1"
  shift
  cargo run --release -p webp --example "$example" --manifest-path "$manifest" -- "$@"
}

run_example animation_encode_bench 1
run_example decode_bench 1 "$fixture"
run_example encode_bench 1 "$fixture"
run_example vp8_encode_bench 1 "$fixture"
cargo run --release -p webp-container --example demux_bench \
  --manifest-path "$manifest" -- 1 "$fixture"
cargo run --release -p webp-container --example mux_editor_bench \
  --manifest-path "$manifest" -- 1 "$fixture"
cargo run --release -p webp --example sharp_yuv_bench --features fuzzing \
  --manifest-path "$manifest" -- 1 "$fixture"
cargo build --release -p webp --example alpha_encode_bench \
  --features fuzzing --manifest-path "$manifest"
