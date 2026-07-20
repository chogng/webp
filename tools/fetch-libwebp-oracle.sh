#!/bin/sh
# Fetch the test-only libwebp oracle at the immutable revision in corpus-lock.
set -eu

repository='https://chromium.googlesource.com/webm/libwebp'
revision='4fa21912338357f89e4fd51cf2368325b59e9bd9'
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

git -C "$destination" fetch --depth=1 origin "$revision"
git -C "$destination" checkout --detach "$revision"
head=$(git -C "$destination" rev-parse HEAD)
if [ "$head" != "$revision" ]; then
    printf '%s\n' "error: resolved $head, expected $revision" >&2
    exit 1
fi

test -f "$destination/tests/fuzzer/fuzz.dict"
printf '%s\n' "libwebp oracle ready at $destination ($revision)"
