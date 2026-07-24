#!/bin/sh
# Verify that the feature-selected upstream smoke corpus is complete.
set -eu

corpus=${1:-third_party/corpus/libwebp-test-data}
selection='tests/corpora/libwebp-test-data-smoke-v1.txt'
repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
checksum_lock="$repo_root/tests/corpora/libwebp-test-data-smoke-v1.sha256"
expected_count=68
. "$repo_root/tools/temporary.sh"
selected_names=$(webp_mktemp_file "$repo_root" webp-smoke-selection)
webp_cleanup_on_exit "$selected_names"

if [ ! -d "$corpus/.git" ]; then
    printf '%s\n' "error: $corpus is not a Git checkout; run tools/fetch-libwebp-test-data.sh" >&2
    exit 1
fi
head=$(git -C "$corpus" rev-parse HEAD)
if [ ! -f "$checksum_lock" ]; then
    printf '%s\n' "error: missing $checksum_lock" >&2
    exit 1
fi
locked_head=$(awk -F ': ' '/^# revision: / { print $2; exit }' "$checksum_lock")
if [ "$head" != "$locked_head" ]; then
    printf '%s\n' "error: corpus revision $head does not match locked revision $locked_head" >&2
    exit 1
fi

count=0
while IFS= read -r fixture || [ -n "$fixture" ]; do
    case "$fixture" in
        '' | \#*) continue ;;
    esac
    case "$fixture" in
        */* | .* | -* | *[!A-Za-z0-9_.-]*)
            printf '%s\n' "error: invalid smoke fixture name: $fixture" >&2
            exit 1
            ;;
        *.webp) ;;
        *)
            printf '%s\n' "error: smoke fixture is not a WebP file: $fixture" >&2
            exit 1
            ;;
    esac
    if [ ! -f "$corpus/$fixture" ]; then
        printf '%s\n' "error: missing $corpus/$fixture" >&2
        exit 1
    fi
    locked_sha=$(awk -v name="$fixture" '
        $2 == name { count += 1; sha = $1 }
        END { if (count == 1) print sha }
    ' "$checksum_lock")
    if [ -z "$locked_sha" ]; then
        printf '%s\n' "error: $fixture must have exactly one checksum entry" >&2
        exit 1
    fi
    actual_sha=$(shasum -a 256 "$corpus/$fixture" | awk '{ print $1 }')
    if [ "$actual_sha" != "$locked_sha" ]; then
        printf '%s\n' "error: checksum mismatch for $fixture" >&2
        exit 1
    fi
    printf '%s\n' "$fixture" >> "$selected_names"
    count=$((count + 1))
done < "$selection"

if [ "$count" -ne "$expected_count" ]; then
    printf '%s\n' "error: found $count smoke entries, expected $expected_count" >&2
    exit 1
fi
unique_count=$(sort -u "$selected_names" | wc -l | tr -d ' ')
if [ "$unique_count" -ne "$count" ]; then
    printf '%s\n' "error: smoke selection contains duplicate fixture names" >&2
    exit 1
fi
checksum_count=$(grep -cv '^#\|^$' "$checksum_lock")
if [ "$checksum_count" -ne "$expected_count" ]; then
    printf '%s\n' "error: found $checksum_count checksum entries, expected $expected_count" >&2
    exit 1
fi
if ! (cd "$corpus" && grep -v '^#\|^$' "$checksum_lock" | shasum -a 256 -c -); then
    printf '%s\n' "error: upstream smoke corpus checksum verification failed" >&2
    exit 1
fi
printf '%s\n' "upstream smoke corpus: $count files at $head"
