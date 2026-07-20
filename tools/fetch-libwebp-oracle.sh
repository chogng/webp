#!/bin/sh
# Fetch the current test-only libwebp oracle without adding it to Git.
set -eu

repository='https://chromium.googlesource.com/webm/libwebp'
branch='main'
destination=${1:-third_party/oracle/libwebp}

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

git -C "$destination" fetch --depth=1 origin "$branch"
git -C "$destination" checkout --detach "origin/$branch"
head=$(git -C "$destination" rev-parse HEAD)
test -f "$destination/tests/fuzzer/fuzz.dict"
printf '%s\n' "libwebp oracle ready at $destination ($branch -> $head)"
