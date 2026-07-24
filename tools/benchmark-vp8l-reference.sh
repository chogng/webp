#!/usr/bin/env bash
# Record the pinned libwebp VP8L reference once per measurement contract.
set -euo pipefail

replace=false
if [[ "${1:-}" == "--replace" ]]; then
  replace=true
  shift
fi
iterations="${1:-5}"
if ! [[ "$iterations" =~ ^[1-9][0-9]*$ ]]; then
  echo "usage: $0 [--replace] [positive iterations]" >&2
  exit 2
fi
if [[ "$#" -gt 1 ]]; then
  echo "usage: $0 [--replace] [positive iterations]" >&2
  exit 2
fi

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
. "$root/tools/temporary.sh"
. "$root/tools/benchmark-vp8l-common.sh"

oracle="$root/third_party/oracle/libwebp"
lockfile="$root/tools/corpus-lock.toml"
results_document="$root/third_party/benchmarks/libwebp/vp8l-encode.md"
if [[ "$replace" == false ]] &&
  rg -q '<!-- BEGIN VP8L ENCODE BENCHMARK DATA' "$results_document"; then
  echo "fixed libwebp reference already exists in $results_document" >&2
  echo "rerun with --replace only after the measurement contract changes" >&2
  exit 1
fi
if [[ ! -f "$oracle/build/libwebp.a" ||
  ! -f "$oracle/build/libsharpyuv.a" ||
  ! -d "$oracle/.git" ]]; then
  echo "fetch and build the pinned libwebp oracle before recording the reference:" >&2
  echo "  tools/fetch-libwebp-oracle.sh" >&2
  exit 1
fi

expected_commit="$(awk -F ' = ' '
  $0 == "[libwebp]" { in_section = 1; next }
  /^\[/ { in_section = 0 }
  in_section && $1 == "commit" {
    value = $2
    gsub(/^"|"$/, "", value)
    print value
    exit
  }
' "$lockfile")"
actual_commit="$(git -C "$oracle" rev-parse HEAD)"
if [[ -z "$expected_commit" || "$actual_commit" != "$expected_commit" ]]; then
  echo "libwebp oracle pin mismatch: expected $expected_commit, found $actual_commit" >&2
  exit 1
fi

vp8l_collect_benchmark_inputs "$root"
scratch="$(webp_mktemp_dir "$root" webp-vp8l-reference)"
webp_cleanup_on_exit "$scratch"
input_manifest="$scratch/inputs.txt"
results="$scratch/libwebp-results.txt"
native="$scratch/libwebp_vp8l_encode_bench"
vp8l_write_benchmark_input_manifest "$input_manifest"

cc -O3 -I"$oracle/src" "$root/tools/libwebp_vp8l_encode_bench.c" \
  "$oracle/build/libwebp.a" "$oracle/build/libsharpyuv.a" -lm -o "$native"

: >"$results"
for level in {0..9}; do
  "$native" 1 "$level" "${vp8l_benchmark_inputs[@]}" >/dev/null
  "$native" "$iterations" "$level" "${vp8l_benchmark_inputs[@]}" |
    tee -a "$results"
done

python3 "$root/tools/update-vp8l-encode-baseline.py" reference \
  --root "$root" \
  --iterations "$iterations" \
  --inputs "$input_manifest" \
  --results "$results" \
  --document "$results_document"
