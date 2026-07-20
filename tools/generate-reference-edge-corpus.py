#!/usr/bin/env python3
"""Generate dimension/alpha edge vectors from the current libwebp oracle."""
from __future__ import annotations

from hashlib import sha256
import json
from pathlib import Path
import subprocess
import tempfile


ROOT = Path("third_party/corpus/reference-edge-v1")
ORACLE = Path("third_party/oracle/libwebp")
SPECS = (
    ("rgb-1x1", 1, 1, False),
    ("rgb-3x5", 3, 5, False),
    ("rgb-17x19", 17, 19, False),
    ("rgb-257x1", 257, 1, False),
    ("rgba-5x7", 5, 7, True),
    ("rgba-19x17", 19, 17, True),
)


def sha256_file(path: Path) -> str:
    return sha256(path.read_bytes()).hexdigest()


def write_input(path: Path, width: int, height: int, alpha: bool) -> None:
    pixels = bytearray()
    for y in range(height):
        for x in range(width):
            pixels.extend(((x * 47 + y * 13) % 256, (x * 11 + y * 71) % 256, (x * 83 + y * 29) % 256))
            if alpha:
                pixels.append((x * 37 + y * 59) % 256)
    if alpha:
        header = (
            f"P7\nWIDTH {width}\nHEIGHT {height}\nDEPTH 4\nMAXVAL 255\n"
            "TUPLTYPE RGB_ALPHA\nENDHDR\n"
        ).encode()
    else:
        header = f"P6\n{width} {height}\n255\n".encode()
    path.write_bytes(header + pixels)


def decoded_rgba(dwebp: Path, webp: Path) -> bytes:
    with tempfile.TemporaryDirectory(prefix="webp-edge-oracle-") as temporary:
        output = Path(temporary) / "decoded.pam"
        subprocess.run([dwebp, str(webp), "-pam", "-o", str(output)], check=True, capture_output=True)
        data = output.read_bytes()
    marker = b"ENDHDR\n"
    header, raw = data.split(marker, 1)
    fields = {}
    for line in header.decode().splitlines()[1:]:
        key, value = line.split(maxsplit=1)
        fields[key] = value
    width, height, depth = (int(fields[name]) for name in ("WIDTH", "HEIGHT", "DEPTH"))
    if len(raw) != width * height * depth or depth not in (3, 4):
        raise RuntimeError(f"unexpected oracle PAM layout for {webp}: depth={depth}, bytes={len(raw)}")
    if depth == 4:
        return raw
    rgba = bytearray()
    for offset in range(0, len(raw), 3):
        rgba.extend(raw[offset : offset + 3])
        rgba.append(255)
    return bytes(rgba)


def toml_string(value: str) -> str:
    return json.dumps(value)


def write_manifest(
    path: Path,
    *,
    vector: Path,
    width: int,
    height: int,
    oracle_revision: str,
    source_sha: str,
    args: list[str],
    rgba_sha: str,
) -> None:
    identifier = vector.stem.replace("_", "-")
    contents = [
        f"id = {toml_string(f'oracle-edge-{identifier}')}",
        f"file = {toml_string(f'../vectors/{vector.name}')}",
        f"sha256 = {toml_string(sha256_file(vector))}",
        'class = "MustAccept"',
        'source = "libwebp main cwebp dimension and alpha edge matrix"',
        'license = "BSD-3-Clause"',
        'codec = "Mixed"',
        'api = "Decode"',
        'features = ["reference-edge", "dimension-alpha"]',
        f"expected_width = {width}",
        f"expected_height = {height}",
        f"expected_rgba_sha256 = {toml_string(rgba_sha)}",
        f"oracle_revision = {toml_string(oracle_revision)}",
        f"source_image_sha256 = {toml_string(source_sha)}",
        "generator_args = [" + ", ".join(toml_string(argument) for argument in args) + "]",
        'notes = "Oracle-decoded canonical RGBA SHA-256 is retained for future public decode golden checks."',
        "",
    ]
    path.write_text("\n".join(contents))


def main() -> None:
    cwebp = ORACLE / "build/cwebp"
    dwebp = ORACLE / "build/dwebp"
    if not (ORACLE / ".git").is_dir() or not cwebp.is_file() or not dwebp.is_file():
        raise SystemExit("build cwebp and dwebp in third_party/oracle/libwebp first")
    oracle_revision = subprocess.check_output(["git", "-C", str(ORACLE), "rev-parse", "HEAD"], text=True).strip()
    inputs, vectors, manifests = (ROOT / name for name in ("inputs", "vectors", "manifests"))
    for directory in (inputs, vectors, manifests):
        directory.mkdir(parents=True, exist_ok=True)

    generated = 0
    for name, width, height, alpha in SPECS:
        source = inputs / f"{name}.pam" if alpha else inputs / f"{name}.ppm"
        write_input(source, width, height, alpha)
        source_sha = sha256_file(source)
        recipes = []
        for quality in (0, 50, 100):
            for method in (0, 6):
                recipes.append((f"lossy-q{quality}-m{method}", ["-q", str(quality), "-m", str(method)]))
        for quality in (0, 100):
            for method in (0, 6):
                recipes.append((f"lossless-q{quality}-m{method}", ["-lossless", "-q", str(quality), "-m", str(method)]))
        recipes.append(("near-lossless-60", ["-lossless", "-near_lossless", "60", "-q", "100", "-m", "6"]))
        for recipe, options in recipes:
            vector = vectors / f"{name}-{recipe}.webp"
            args = ["cwebp", *options]
            subprocess.run([cwebp, "-quiet", *options, source, "-o", vector], check=True)
            rgba_sha = sha256(decoded_rgba(dwebp, vector)).hexdigest()
            write_manifest(
                manifests / f"{vector.stem}.toml",
                vector=vector,
                width=width,
                height=height,
                oracle_revision=oracle_revision,
                source_sha=source_sha,
                args=args,
                rgba_sha=rgba_sha,
            )
            generated += 1
    print(f"generated {generated} reference edge vectors in {ROOT} ({oracle_revision})")


if __name__ == "__main__":
    main()
