#!/bin/sh
# Regenerate the current libwebp cwebp matrix for direct Rust API tests.
set -eu

root=${1:-third_party/corpus/reference-v1}
oracle=${2:-third_party/oracle/libwebp}
cwebp="$oracle/build/cwebp"
input="$oracle/examples/test_ref.ppm"

if [ ! -x "$cwebp" ]; then
    printf '%s\n' "error: missing $cwebp; configure and build the oracle first" >&2
    exit 1
fi
if [ ! -f "$input" ]; then
    printf '%s\n' "error: missing oracle input $input" >&2
    exit 1
fi

mkdir -p "$root/inputs" "$root/lossy" "$root/lossless"
cp "$input" "$root/inputs/test_ref.ppm"

for quality in 0 10 25 50 75 90 100; do
    for method in 0 3 6; do
        "$cwebp" -quiet -q "$quality" -m "$method" "$root/inputs/test_ref.ppm" \
            -o "$root/lossy/q${quality}-m${method}.webp"
    done
done

for quality in 0 25 50 75 100; do
    for method in 0 3 6; do
        "$cwebp" -quiet -lossless -q "$quality" -m "$method" "$root/inputs/test_ref.ppm" \
            -o "$root/lossless/q${quality}-m${method}.webp"
    done
done

printf '%s\n' "generated 36 reference vectors in $root"
