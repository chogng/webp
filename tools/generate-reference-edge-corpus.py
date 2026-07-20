#!/usr/bin/env python3
"""Generate dimension/alpha edge vectors from the current libwebp oracle."""
from __future__ import annotations

from pathlib import Path
import subprocess


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

def main() -> None:
    cwebp = ORACLE / "build/cwebp"
    if not (ORACLE / ".git").is_dir() or not cwebp.is_file():
        raise SystemExit("build cwebp in third_party/oracle/libwebp first")
    inputs, vectors = (ROOT / name for name in ("inputs", "vectors"))
    for directory in (inputs, vectors):
        directory.mkdir(parents=True, exist_ok=True)

    generated = 0
    for name, width, height, alpha in SPECS:
        source = inputs / f"{name}.pam" if alpha else inputs / f"{name}.ppm"
        write_input(source, width, height, alpha)
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
            subprocess.run([cwebp, "-quiet", *options, source, "-o", vector], check=True)
            generated += 1
    print(f"generated {generated} reference edge vectors in {ROOT}")


if __name__ == "__main__":
    main()
