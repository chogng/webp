#!/bin/sh
# Generate Rust-testkit sidecars for the selected upstream conformance vectors.
set -eu

corpus=${1:-third_party/corpus/libwebp-test-data}
selection=${2:-tests/corpora/libwebp-test-data-smoke-v1.txt}
manifest_root="$corpus/manifests"

if [ ! -d "$corpus/.git" ]; then
    printf '%s\n' "error: $corpus is not a Git checkout; run tools/fetch-libwebp-test-data.sh" >&2
    exit 1
fi
if [ ! -f "$selection" ]; then
    printf '%s\n' "error: missing selection $selection" >&2
    exit 1
fi

mkdir -p "$manifest_root"
revision=$(git -C "$corpus" rev-parse HEAD)
count=0
while IFS= read -r fixture || [ -n "$fixture" ]; do
    case "$fixture" in
        '' | \#*) continue ;;
    esac
    file="$corpus/$fixture"
    if [ ! -f "$file" ]; then
        printf '%s\n' "error: missing selected upstream vector $file" >&2
        exit 1
    fi
    stem=$(printf '%s' "$fixture" | tr '/.' '--')
    sha=$(shasum -a 256 "$file" | awk '{print $1}')
    cat > "$manifest_root/${stem}.toml" <<EOF
id = "upstream-${stem}"
file = "../${fixture}"
sha256 = "$sha"
class = "ImplementationDefined"
source = "libwebp-test-data main selected upstream vector"
license = "BSD-3-Clause"
codec = "Mixed"
api = "Decode"
features = ["upstream-conformance", "selected-smoke"]
oracle_revision = "$revision"
notes = "Integrity sidecar for a selected upstream vector. Promote to MustAccept with dimensions/pixel golden once the corresponding public decoder path is implemented."
EOF
    count=$((count + 1))
done < "$selection"
printf '%s\n' "indexed $count upstream smoke vectors at $revision"
