#!/bin/sh
# Add a minimized historical WebP regression to the committed Rust fixture set.
set -eu

if [ "$#" -ne 4 ]; then
    printf '%s\n' "usage: $0 <input.webp> <id> <issue-or-source> <license>" >&2
    exit 2
fi

input=$1
id=$2
issue=$3
license=$4
root=${WEBP_REGRESSION_ROOT:-tests}

case "$id" in
    *[!a-z0-9-]* | '')
        printf '%s\n' "error: id must use lowercase letters, digits, and hyphens" >&2
        exit 2
        ;;
esac
if [ ! -f "$input" ]; then
    printf '%s\n' "error: missing input $input" >&2
    exit 1
fi

fixture="$root/fixtures/regressions/${id}.webp"
manifest="$root/manifests/${id}.toml"
if [ -e "$fixture" ] || [ -e "$manifest" ]; then
    printf '%s\n' "error: regression id already exists: $id" >&2
    exit 1
fi

mkdir -p "$root/fixtures/regressions" "$root/manifests"
cp "$input" "$fixture"
sha=$(shasum -a 256 "$fixture" | awk '{print $1}')
cat > "$manifest" <<EOF
id = "$id"
file = "../fixtures/regressions/${id}.webp"
sha256 = "$sha"
class = "MustReject"
source = "regression: $issue"
license = "$license"
codec = "Container"
features = ["historical-regression", "public-api", "no-panic"]
notes = "Minimized reproducer promoted from $issue. Update class/API/golden fields if later behavior is acceptance."
EOF

printf '%s\n' "added regression fixture $fixture and manifest $manifest"
