#!/usr/bin/env bash
# Compare the public Rust VP8L decoder with pinned libwebp on CLIC images.
set -euo pipefail

iterations="${1:-1}"
jobs="${2:-4}"
if ! [[ "$iterations" =~ ^[1-9][0-9]*$ ]] || ! [[ "$jobs" =~ ^[1-9][0-9]*$ ]]; then
  echo "usage: $0 [positive iterations] [positive encoding jobs]" >&2
  exit 2
fi

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
manifest="$root/third_party/benchdata/clic/validation-manifest.json"
input_root="$root/third_party/benchdata/clic/validation-png"
output_root="$root/third_party/benchdata/clic/vp8l-lossless-exact"
oracle="$root/third_party/oracle/libwebp"
lockfile="$root/tools/corpus-lock.toml"
cwebp="$oracle/build/cwebp"

for command in cargo cc jq python3 xargs; do
  if ! command -v "$command" >/dev/null 2>&1; then
    echo "required command not found: $command" >&2
    exit 1
  fi
done
if [[ ! -f "$manifest" || ! -d "$input_root" ]]; then
  echo "fetch the pinned CLIC validation corpus before benchmarking:" >&2
  echo "  tools/fetch-clic-validation.sh" >&2
  exit 1
fi
if [[ ! -x "$cwebp" || ! -f "$oracle/build/libwebp.a" || ! -d "$oracle/.git" ]]; then
  echo "fetch and build the pinned libwebp oracle before benchmarking:" >&2
  echo "  tools/fetch-libwebp-oracle.sh" >&2
  exit 1
fi

lock_value() {
  local section="$1"
  local key="$2"
  awk -F ' = ' -v section="$section" -v key="$key" '
    $0 == "[" section "]" { in_section = 1; next }
    /^\[/ { in_section = 0 }
    in_section && $1 == key {
      value = $2
      gsub(/^"|"$/, "", value)
      print value
      exit
    }
  ' "$lockfile"
}

expected_commit="$(lock_value libwebp commit)"
actual_commit="$(git -C "$oracle" rev-parse HEAD)"
if [[ -z "$expected_commit" || "$actual_commit" != "$expected_commit" ]]; then
  echo "libwebp oracle pin mismatch: expected $expected_commit, found $actual_commit" >&2
  exit 1
fi

cd "$root"
python3 tools/verify-clic-validation.py
mkdir -p "$output_root"

# Produce three structurally different exact-lossless streams per source. The
# cache is ignored by Git, and existing outputs make interrupted runs resumable.
jq -j '
  .images[] |
  .id as $id | .file as $file |
  [0, 3, 6][] as $method |
  $id, "\u0000", $file, "\u0000", ($method | tostring), "\u0000"
' "$manifest" | xargs -0 -n 3 -P "$jobs" sh -c '
  input_root=$1
  output_root=$2
  cwebp=$3
  id=$4
  relative=$5
  method=$6
  output="$output_root/$id-m$method.webp"
  if [ -f "$output" ]; then
    exit 0
  fi
  temporary="$output.tmp.$$"
  trap '\''rm -f -- "$temporary"'\'' EXIT HUP INT TERM
  "$cwebp" -quiet -mt -lossless -exact -m "$method" \
    "$input_root/$relative" -o "$temporary"
  mv -- "$temporary" "$output"
  trap - EXIT HUP INT TERM
' benchmark-vp8l-clic "$input_root" "$output_root" "$cwebp"

source_count="$(jq '.images | length' "$manifest")"
expected_count="$((source_count * 3))"
actual_count="$(find "$output_root" -type f -name '*.webp' | wc -l | tr -d ' ')"
if [[ "$actual_count" != "$expected_count" ]]; then
  echo "expected $expected_count generated WebP files, found $actual_count" >&2
  exit 1
fi
for method in 0 3 6; do
  method_count="$(find "$output_root" -type f -name "*-m$method.webp" | wc -l | tr -d ' ')"
  if [[ "$method_count" != "$source_count" ]]; then
    echo "method $method: expected $source_count files, found $method_count" >&2
    exit 1
  fi
done

scratch="$(mktemp -d "${TMPDIR:-/tmp}/webp-vp8l-clic-bench.XXXXXX")"
cleanup() {
  rm -r -- "$scratch"
}
trap cleanup EXIT HUP INT TERM
native="$scratch/libwebp_decode_bench"
cc -O3 -I"$oracle/src" "$root/tools/libwebp_decode_bench.c" \
  "$oracle/build/libwebp.a" -o "$native"
cargo build --release -p webp --example decode_bench --manifest-path "$root/webp-rs/Cargo.toml"
rust="$root/target/release/examples/decode_bench"

echo "oracle_commit=$actual_commit sources=$source_count streams=$actual_count"
echo "aggregate (all cwebp methods)"
all_inputs=("$output_root"/*.webp)
"$native" "$iterations" "${all_inputs[@]}"
"$rust" "$iterations" "${all_inputs[@]}"

for method in 0 3 6; do
  echo "method=$method"
  method_inputs=("$output_root"/*-m"$method".webp)
  "$native" "$iterations" "${method_inputs[@]}"
  "$rust" "$iterations" "${method_inputs[@]}"
done
