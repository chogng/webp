#!/bin/sh
# Verify that the feature-selected upstream smoke corpus is complete and pinned.
set -eu

revision='06ddd96e276c2c638a72d39d3c0f340afd61978c'
corpus=${1:-third_party/corpus/libwebp-test-data}
selection='tests/corpora/libwebp-test-data-smoke-v1.txt'
expected_count=64

if [ ! -d "$corpus/.git" ]; then
    printf '%s\n' "error: $corpus is not a Git checkout; run tools/fetch-libwebp-test-data.sh" >&2
    exit 1
fi
head=$(git -C "$corpus" rev-parse HEAD)
if [ "$head" != "$revision" ]; then
    printf '%s\n' "error: corpus is at $head, expected $revision" >&2
    exit 1
fi

count=0
while IFS= read -r fixture || [ -n "$fixture" ]; do
    case "$fixture" in
        '' | \#*) continue ;;
    esac
    if [ ! -f "$corpus/$fixture" ]; then
        printf '%s\n' "error: missing $corpus/$fixture" >&2
        exit 1
    fi
    count=$((count + 1))
done < "$selection"

if [ "$count" -ne "$expected_count" ]; then
    printf '%s\n' "error: found $count smoke entries, expected $expected_count" >&2
    exit 1
fi
printf '%s\n' "upstream smoke corpus: $count files at $revision"
