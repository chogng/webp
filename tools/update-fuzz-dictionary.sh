#!/bin/sh
# Refresh the checked-in fuzz dictionary from the current test-only oracle.
set -eu

oracle=${1:-third_party/oracle/libwebp}
source="$oracle/tests/fuzzer/fuzz.dict"
destination='fuzz/dictionaries/webp.dict'

if [ ! -d "$oracle/.git" ] || [ ! -f "$source" ]; then
    printf '%s\n' "error: missing libwebp oracle dictionary; run tools/fetch-libwebp-oracle.sh" >&2
    exit 1
fi

cp "$source" "$destination"
revision=$(git -C "$oracle" rev-parse HEAD)
printf '%s\n' "updated $destination from libwebp $revision"
