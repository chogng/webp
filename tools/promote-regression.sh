#!/bin/sh
# Add a minimized historical WebP regression to the committed Rust fixture set.
set -eu

if [ "$#" -ne 4 ]; then
    printf '%s\n' "usage: $0 <input.webp> <id> <issue-or-source> <license>" >&2
    exit 2
fi

input=$1
id=$2
issue=$3
license=$4
root=${WEBP_REGRESSION_ROOT:-tests}

case "$id" in
    *[!a-z0-9-]* | '')
        printf '%s\n' "error: id must use lowercase letters, digits, and hyphens" >&2
        exit 2
        ;;
esac
if [ ! -f "$input" ]; then
    printf '%s\n' "error: missing input $input" >&2
    exit 1
fi

fixture="$root/fixtures/regressions/${id}.webp"
if [ -e "$fixture" ]; then
    printf '%s\n' "error: regression id already exists: $id" >&2
    exit 1
fi

mkdir -p "$root/fixtures/regressions"
cp "$input" "$fixture"

printf '%s\n' "added regression fixture $fixture from $issue ($license); add its public API test in crates/webp"
