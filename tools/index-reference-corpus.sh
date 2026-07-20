#!/bin/sh
# Generate Rust-testkit sidecars for current libwebp reference encoder outputs.
set -eu

root=${1:-third_party/corpus/reference-v1}
oracle=${2:-third_party/oracle/libwebp}
manifest_root="$root/manifests"
mkdir -p "$manifest_root"

if [ ! -d "$oracle/.git" ]; then
    printf '%s\n' "error: $oracle is not a Git checkout; run tools/fetch-libwebp-oracle.sh" >&2
    exit 1
fi
oracle_revision=$(git -C "$oracle" rev-parse HEAD)

find "$root/lossy" "$root/lossless" -type f -name '*.webp' | sort | while IFS= read -r file; do
    relative=${file#"$root/"}
    stem=$(printf '%s' "$relative" | tr '/.' '--')
    sha=$(shasum -a 256 "$file" | awk '{print $1}')
    class=MustAccept
    printf '%s\n' "id = \"oracle-${stem}\"" > "$manifest_root/${stem}.toml"
    printf '%s\n' "file = \"../${relative}\"" >> "$manifest_root/${stem}.toml"
    printf '%s\n' "sha256 = \"${sha}\"" >> "$manifest_root/${stem}.toml"
    printf '%s\n' "class = \"${class}\"" >> "$manifest_root/${stem}.toml"
    printf '%s\n' 'source = "libwebp main cwebp reference matrix"' >> "$manifest_root/${stem}.toml"
    printf '%s\n' 'license = "BSD-3-Clause"' >> "$manifest_root/${stem}.toml"
    printf '%s\n' 'codec = "Mixed"' >> "$manifest_root/${stem}.toml"
    printf '%s\n' 'api = "Decode"' >> "$manifest_root/${stem}.toml"
    printf '%s\n' 'expected_width = 128' >> "$manifest_root/${stem}.toml"
    printf '%s\n' 'expected_height = 128' >> "$manifest_root/${stem}.toml"
    printf '%s\n' "notes = \"Generated from examples/test_ref.ppm; oracle revision ${oracle_revision}.\"" >> "$manifest_root/${stem}.toml"
done
