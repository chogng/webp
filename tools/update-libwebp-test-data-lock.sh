#!/bin/sh
# Advance the libwebp-test-data pin to the current tracking-branch tip.
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
lockfile="$repo_root/tools/corpus-lock.toml"

lock_value() {
    section=$1
    key=$2
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

repository=$(lock_value libwebp_test_data source_url)
branch=$(lock_value libwebp_test_data tracking_branch)
current=$(lock_value libwebp_test_data commit)
next=$(git ls-remote "$repository" "refs/heads/$branch" | awk 'NR == 1 { print $1 }')

if [ -z "$next" ]; then
    printf '%s\n' "error: could not resolve $repository $branch" >&2
    exit 1
fi
if [ "$current" = "$next" ]; then
    printf '%s\n' "libwebp-test-data is already pinned to $current"
    exit 0
fi

temporary="$lockfile.tmp"
awk -v next="$next" '
    $0 == "[libwebp_test_data]" { in_section = 1 }
    /^\[/ && $0 != "[libwebp_test_data]" { in_section = 0 }
    in_section && /^commit = / { print "commit = \"" next "\""; next }
    { print }
' "$lockfile" > "$temporary"
mv "$temporary" "$lockfile"
printf '%s\n' "updated libwebp-test-data pin: $current -> $next"
