#!/bin/sh
# Fetch the current upstream WebP conformance corpus without adding it to Git.
set -eu

repository='https://chromium.googlesource.com/webm/libwebp-test-data'
branch='main'
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

git -C "$destination" fetch --depth=1 origin "$branch"
git -C "$destination" checkout --detach "origin/$branch"
head=$(git -C "$destination" rev-parse HEAD)
printf '%s\n' "libwebp-test-data ready at $destination ($branch -> $head)"
