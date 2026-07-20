#!/bin/sh
# Generate a minimal real animation vector plus a Rust-testkit sidecar.
set -eu

root=${1:-third_party/corpus/animation-v1}
oracle=${2:-third_party/oracle/libwebp}
img2webp="$oracle/build/img2webp"
source="$oracle/examples/test_ref.ppm"

if [ ! -x "$img2webp" ] || [ ! -d "$oracle/.git" ] || [ ! -f "$source" ]; then
    printf '%s\n' "error: build img2webp in $oracle first" >&2
    exit 1
fi

mkdir -p "$root/inputs" "$root/manifests"
cp "$source" "$root/inputs/frame-a.ppm"
cp "$source" "$root/inputs/frame-b.ppm"
# The first pixel starts at byte 15 in this P6 128x128 source. Change its red
# channel so the encoder must retain two distinct animation frames.
printf '\000' | dd of="$root/inputs/frame-b.ppm" bs=1 seek=15 conv=notrunc 2>/dev/null

output="$root/two-frame-loop.webp"
"$img2webp" -loop 0 "$root/inputs/frame-a.ppm" -d 40 "$root/inputs/frame-b.ppm" -o "$output"

sha=$(shasum -a 256 "$output" | awk '{print $1}')
revision=$(git -C "$oracle" rev-parse HEAD)
cat > "$root/manifests/two-frame-loop.toml" <<EOF
id = "oracle-animation-two-frame-loop"
file = "../two-frame-loop.webp"
sha256 = "$sha"
class = "MustAccept"
source = "libwebp main img2webp animation matrix"
license = "BSD-3-Clause"
codec = "Mixed"
api = "ReadInfo"
features = ["animation", "two-frame", "loop"]
expected_width = 128
expected_height = 128
oracle_revision = "$revision"
generator_args = ["img2webp", "-loop", "0", "frame-a.ppm", "-d", "40", "frame-b.ppm"]
notes = "Two distinct 128x128 frames; durations are 100 ms and 40 ms."
EOF

printf '%s\n' "generated animation vector in $root ($revision)"
