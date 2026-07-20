#!/bin/sh
# Fetch the pinned upstream WebP conformance corpus without adding it to Git.
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
commit=$(lock_value libwebp_test_data commit)
if [ -z "$repository" ] || [ -z "$commit" ]; then
    printf '%s\n' "error: libwebp_test_data source_url or commit is missing from $lockfile" >&2
    exit 1
fi

destination=${1:-third_party/corpus/libwebp-test-data}

if [ -e "$destination" ]; then
    if [ ! -d "$destination/.git" ]; then
        printf '%s\n' "error: $destination exists but is not a Git checkout" >&2
        exit 1
    fi
    origin=$(git -C "$destination" remote get-url origin)
    if [ "$origin" != "$repository" ]; then
        printf '%s\n' "error: $destination has unexpected origin: $origin" >&2
        exit 1
    fi
else
    mkdir -p "$(dirname "$destination")"
    git clone --no-checkout "$repository" "$destination"
fi

git -C "$destination" fetch --depth=1 origin "$commit"
git -C "$destination" checkout --detach "$commit"
head=$(git -C "$destination" rev-parse HEAD)
if [ "$head" != "$commit" ]; then
    printf '%s\n' "error: expected $commit but checked out $head" >&2
    exit 1
fi
printf '%s\n' "libwebp-test-data ready at $destination ($head)"
