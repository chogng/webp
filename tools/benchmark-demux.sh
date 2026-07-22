#!/usr/bin/env bash
# Measure strict public Rust Demuxer parsing on the pinned reference corpus.
set -euo pipefail

iterations="${1:-10000}"
if ! [[ "$iterations" =~ ^[1-9][0-9]*$ ]]; then
  echo "usage: $0 [positive iterations]" >&2
  exit 2
fi

root="$(cd "$(dirname "$0")/.." && pwd)"
reference="$root/third_party/corpus/reference-v1"
animation="$root/third_party/corpus/animation-v1"

if [[ ! -d "$reference/manifests" || ! -d "$animation/manifests" ]]; then
  echo "generate the pinned reference corpus before benchmarking:" >&2
  echo "  bash tools/generate-reference-corpus.sh" >&2
  exit 1
fi

inputs=()
for corpus in "$reference" "$animation"; do
  for manifest in "$corpus"/manifests/*.toml; do
    if rg -q '^class = "MustAccept"$' "$manifest"; then
      file="$(sed -n 's|^file = "../\(.*\)"|\1|p' "$manifest")"
      inputs+=("$corpus/$file")
    fi
  done
done
if [[ "${#inputs[@]}" -eq 0 ]]; then
  echo "no accepted container benchmark inputs found" >&2
  exit 1
fi

cargo run --quiet --release -p webp-container --example demux_bench \
  --manifest-path "$root/webp-rs/Cargo.toml" -- "$iterations" "${inputs[@]}"
