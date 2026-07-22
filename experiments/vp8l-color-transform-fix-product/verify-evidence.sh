#!/bin/sh
set -eu

evidence_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
before="$evidence_dir/before-102.tsv"
after="$evidence_dir/after-102.tsv"
decoder_306="$evidence_dir/decoder-306.txt"
synthetic="$evidence_dir/reproducer-129x129.txt"
synthetic_webp="$evidence_dir/reproducer-129x129.webp"

before_sha=$(shasum -a 256 "$before" | awk '{print $1}')
after_sha=$(shasum -a 256 "$after" | awk '{print $1}')
synthetic_sha=$(shasum -a 256 "$synthetic_webp" | awk '{print $1}')

test "$before_sha" = "0094c10d3a8417e786e206617855b6a9f9fb4e7123e1cb63bd20bef2f4c45f82"
test "$after_sha" = "994a4afabb52d94e65678ce15de57a09c35c279b53566cf94f26137201bd7b34"
test "$synthetic_sha" = "ce513d7c3ebaa1abd8436aabbda9306ad71fa69e74303be7af34b6e552a15d9f"

awk -F '\t' '
  NR == 1 { next }
  {
    rows++
    bytes += $6
    if ($7 != "ok") project_fail++
    if ($11 != "ok") dwebp_fail++
  }
  END {
    if (rows != 102 || project_fail != 101 || dwebp_fail != 101 || bytes != 661692326) exit 1
  }
' "$before"

awk -F '\t' '
  NR == 1 { next }
  {
    rows++
    bytes += $6
    if ($7 != "ok") project_fail++
    if ($11 != "ok") dwebp_fail++
  }
  END {
    if (rows != 102 || project_fail != 0 || dwebp_fail != 0 || bytes != 661692326) exit 1
  }
' "$after"

awk -F '\t' '
  FNR == NR {
    if (FNR > 1) {
      rgba[$1] = $4
      hash[$1] = $5
      bytes[$1] = $6
      before_rows++
    }
    next
  }
  FNR > 1 {
    after_rows++
    if (!($1 in rgba) || rgba[$1] != $4 || bytes[$1] != $6) mismatch++
    if (hash[$1] == $5) {
      unchanged++
      unchanged_id = $1
    }
    seen[$1] = 1
  }
  END {
    for (id in rgba) if (!(id in seen)) missing++
    if (before_rows != 102 || after_rows != 102 || mismatch != 0 || missing != 0 ||
        unchanged != 1 || unchanged_id != "clic-validation-098") exit 1
  }
' "$before" "$after"

awk -F '\t' '
  NR == 2 {
    if ($1 != "clic-validation-000" ||
        $5 != "9c68ff67c15c5d3df7daece012d3e9f0b8d7fd5e10e74e291d3aab1eebe97d80" ||
        $6 != 8196184 || $7 != "ok" || $11 != "ok") exit 1
  }
' "$after"

grep -q '^validation backend=rust files=306 exact_rgba=true mismatches=0 decode_errors=0 checksum_fnv1a64=16f1b6ca2415e2d6$' "$decoder_306"
grep -q '^validation backend=image-webp files=306 exact_rgba=true mismatches=0 decode_errors=0 checksum_fnv1a64=16f1b6ca2415e2d6$' "$decoder_306"
grep -q '^output_sha256[[:space:]]ce513d7c3ebaa1abd8436aabbda9306ad71fa69e74303be7af34b6e552a15d9f$' "$synthetic"
grep -q '^output_bytes[[:space:]]50704$' "$synthetic"
grep -q '^project_status[[:space:]]rgba_equal=true$' "$synthetic"
grep -q '^dwebp_status[[:space:]]ok$' "$synthetic"

printf '%s\n' \
  "before: 102 rows, project failures=101, dwebp failures=101" \
  "after: 102 rows, project failures=0, dwebp failures=0" \
  "paired: RGBA SHA and bytes unchanged; total bytes=661692326; no-transform unchanged=1" \
  "first after: sha256=9c68ff67c15c5d3df7daece012d3e9f0b8d7fd5e10e74e291d3aab1eebe97d80 bytes=8196184" \
  "decoder gate: rust/image-webp exact=306/306 checksum=16f1b6ca2415e2d6" \
  "synthetic: 129x129, both decoders exact, bytes=50704" \
  "evidence assertions: ok"
