#!/usr/bin/env bash
# Shared corpus selection for the VP8L encoder benchmarks.

vp8l_collect_benchmark_inputs() {
  local root="$1"
  local corpus="$root/third_party/corpus/libwebp-test-data"
  local manifest
  local file

  if [[ ! -d "$corpus/manifests" ]]; then
    echo "fetch the pinned corpus before benchmarking:" >&2
    echo "  tools/fetch-libwebp-test-data.sh" >&2
    return 1
  fi

  vp8l_benchmark_inputs=()
  for manifest in "$corpus"/manifests/*.toml; do
    if rg -q '^class = "MustAccept"$' "$manifest" &&
      rg -q '^codec = "VP8L"$' "$manifest" &&
      rg -q '^api = "Decode"$' "$manifest"; then
      file="$(sed -n 's|^file = "../\(.*\)"|\1|p' "$manifest")"
      vp8l_benchmark_inputs+=("$corpus/$file")
    fi
  done
  if [[ "${#vp8l_benchmark_inputs[@]}" -eq 0 ]]; then
    echo "no accepted VP8L benchmark inputs found" >&2
    return 1
  fi
}

vp8l_write_benchmark_input_manifest() {
  local output="$1"
  printf '%s\n' "${vp8l_benchmark_inputs[@]}" >"$output"
}
