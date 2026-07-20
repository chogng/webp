#!/bin/sh
# Generate Rust-testkit sidecars for the selected upstream conformance vectors.
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
corpus=${1:-third_party/corpus/libwebp-test-data}
selection=${2:-tests/corpora/libwebp-test-data-smoke-v1.txt}
manifest_root="$corpus/manifests"
dwebp=${DWEBP:-third_party/oracle/libwebp/build/dwebp}
lockfile="$repo_root/tools/corpus-lock.toml"

lock_value() {
    section=$1
    key=$2
    awk -F ' = ' -v section="$section" -v key="$key" '
        $0 == "[" section "]" { in_section = 1; next }
        /^\[/ { in_section = 0 }
        in_section && $1 == key {
            value = $2
            sub(/[[:space:]]+#.*/, "", value)
            gsub(/^"|"$/, "", value)
            print value
            exit
        }
    ' "$lockfile"
}

if [ ! -d "$corpus/.git" ]; then
    printf '%s\n' "error: $corpus is not a Git checkout; run tools/fetch-libwebp-test-data.sh" >&2
    exit 1
fi
if [ ! -f "$selection" ]; then
    printf '%s\n' "error: missing selection $selection" >&2
    exit 1
fi
if [ ! -x "$dwebp" ]; then
    printf '%s\n' "error: missing executable libwebp oracle dwebp at $dwebp" >&2
    exit 1
fi

oracle_root=$(CDPATH= cd -- "$(dirname -- "$dwebp")/.." && pwd)
if [ ! -d "$oracle_root/.git" ]; then
    printf '%s\n' "error: $dwebp is not built from a Git checkout" >&2
    exit 1
fi
oracle_revision=$(git -C "$oracle_root" rev-parse HEAD)
locked_oracle_revision=$(lock_value libwebp commit)
if [ "$oracle_revision" != "$locked_oracle_revision" ]; then
    printf '%s\n' "error: oracle revision $oracle_revision does not match lock $locked_oracle_revision" >&2
    exit 1
fi
if ! git -C "$oracle_root" diff --quiet -- || \
   ! git -C "$oracle_root" diff --cached --quiet --; then
    printf '%s\n' "error: oracle checkout has tracked modifications" >&2
    exit 1
fi

oracle_rgba() {
    fixture=$1
    output=$2
    "$dwebp" "$fixture" -pam -o "$output" >/dev/null 2>&1
    python3 - "$output" <<'PY'
from hashlib import sha256
from pathlib import Path
import sys

header, pixels = Path(sys.argv[1]).read_bytes().split(b"ENDHDR\n", 1)
fields = dict(line.split(maxsplit=1) for line in header.splitlines()[1:])
width = int(fields[b"WIDTH"])
height = int(fields[b"HEIGHT"])
depth = int(fields[b"DEPTH"])
if depth != 4 or len(pixels) != width * height * depth:
    raise SystemExit("error: oracle output is not canonical RGBA PAM")
print(width, height, sha256(pixels).hexdigest())
PY
}

is_m1_lossless() {
    case "$1" in
        lossless1.webp | lossless2.webp | lossless3.webp | lossless4.webp | \
        lossless_big_random_alpha.webp | lossless_color_transform.webp | \
        lossless_vec_1_*.webp | lossless_vec_2_*.webp | \
        color_cache_bits_11.webp | dual_transform.webp | one_color_no_palette.webp)
            return 0
            ;;
        *)
            return 1
            ;;
    esac
}

mkdir -p "$manifest_root"
revision=$(git -C "$corpus" rev-parse HEAD)
temporary=$(mktemp -d "${TMPDIR:-/tmp}/webp-upstream-index.XXXXXX")
trap 'rm -rf "$temporary"' EXIT HUP INT TERM
count=0
while IFS= read -r fixture || [ -n "$fixture" ]; do
    case "$fixture" in
        '' | \#*) continue ;;
    esac
    case "$fixture" in
        */* | .* | -* | *[!A-Za-z0-9_.-]*)
            printf '%s\n' "error: invalid smoke fixture name: $fixture" >&2
            exit 1
            ;;
        *.webp) ;;
        *)
            printf '%s\n' "error: smoke fixture is not a WebP file: $fixture" >&2
            exit 1
            ;;
    esac
    file="$corpus/$fixture"
    if [ ! -f "$file" ]; then
        printf '%s\n' "error: missing selected upstream vector $file" >&2
        exit 1
    fi
    stem=$(printf '%s' "$fixture" | tr '/.' '--')
    sha=$(shasum -a 256 "$file" | awk '{print $1}')
    if is_m1_lossless "$fixture"; then
        set -- $(oracle_rgba "$file" "$temporary/$stem.pam")
        width=$1
        height=$2
        rgba_sha=$3
        cat > "$manifest_root/${stem}.toml" <<EOF
id = "upstream-${stem}"
file = "../${fixture}"
sha256 = "$sha"
class = "MustAccept"
source = "libwebp-test-data pinned lossless conformance vector"
license = "BSD-3-Clause"
codec = "VP8L"
api = "Decode"
features = ["upstream-conformance", "selected-smoke", "vp8l", "lossless"]
expected_width = $width
expected_height = $height
expected_rgba_sha256 = "$rgba_sha"
oracle_revision = "$oracle_revision"
notes = "Canonical RGBA8 from libwebp dwebp at the recorded oracle revision."
EOF
    else
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
    fi
    count=$((count + 1))
done < "$selection"
manifest_count=$(find "$manifest_root" -type f -name '*.toml' | wc -l | tr -d ' ')
if [ "$manifest_count" -ne "$count" ]; then
    printf '%s\n' "error: manifest directory contains stale or unselected TOML files" >&2
    exit 1
fi
if find "$manifest_root" -type l | grep -q .; then
    printf '%s\n' "error: manifest directory must not contain symbolic links" >&2
    exit 1
fi
printf '%s\n' "indexed $count upstream smoke vectors at $revision (M1 oracle $oracle_revision)"
