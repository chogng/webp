#!/bin/sh
# Report whether the configured upstream tracking branch has a newer commit.
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
available=$(git ls-remote "$repository" "refs/heads/$branch" | awk 'NR == 1 { print $1 }')

if [ -z "$available" ]; then
    printf '%s\n' "error: could not resolve $repository $branch" >&2
    exit 1
fi

printf '%s\n' "current=$current"
printf '%s\n' "available=$available"
if [ "$current" = "$available" ]; then
    printf '%s\n' "update_available=false"
else
    printf '%s\n' "update_available=true"
fi
