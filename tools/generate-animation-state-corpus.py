#!/usr/bin/env python3
"""Generate animation state vectors with explicit mux frame controls."""
from __future__ import annotations

from pathlib import Path
import subprocess


ROOT = Path("third_party/corpus/animation-v1")
ORACLE = Path("third_party/oracle/libwebp")
WIDTH, HEIGHT = 128, 96


def write_ppm(path: Path, width: int, height: int, color: tuple[int, int, int]) -> None:
    path.write_bytes(f"P6\n{width} {height}\n255\n".encode() + bytes(color) * (width * height))

def main() -> None:
    cwebp = ORACLE / "build/cwebp"
    webpmux = ORACLE / "build/webpmux"
    if not (ORACLE / ".git").is_dir() or not cwebp.is_file() or not webpmux.is_file():
        raise SystemExit("build cwebp and webpmux in third_party/oracle/libwebp first")
    inputs, frames = (ROOT / name for name in ("state-inputs", "state-frames"))
    for directory in (inputs, frames):
        directory.mkdir(parents=True, exist_ok=True)

    source_specs = {
        "base": (WIDTH, HEIGHT, (220, 35, 35)),
        "patch": (64, 48, (35, 35, 220)),
        "final": (32, 48, (35, 220, 35)),
    }
    encoded = {}
    for name, (width, height, color) in source_specs.items():
        source = inputs / f"{name}.ppm"
        output = frames / f"{name}.webp"
        write_ppm(source, width, height, color)
        subprocess.run([cwebp, "-quiet", "-lossless", source, "-o", output], check=True)
        encoded[name] = output

    variants = {
        "animation-blend-loop-one": {
            "frames": [
                ("base", 100, 0, 0, 0, "+b"),
                ("patch", 40, 32, 24, 0, "+b"),
            ],
            "loop": 1,
            "background": "255,10,20,30",
        },
        "animation-dispose-no-blend-loop-zero": {
            "frames": [
                ("base", 100, 0, 0, 0, "+b"),
                ("patch", 40, 32, 24, 1, "-b"),
                ("final", 80, 48, 24, 0, "-b"),
            ],
            "loop": 0,
            "background": "255,10,20,30",
        },
    }

    for name, variant in variants.items():
        output = ROOT / f"{name}.webp"
        command = [webpmux]
        for frame, duration, x, y, dispose, blend in variant["frames"]:
            controls = f"+{duration}+{x}+{y}+{dispose}{blend}"
            command.extend(["-frame", encoded[frame], controls])
        command.extend(["-loop", str(variant["loop"]), "-bgcolor", variant["background"], "-o", output])
        subprocess.run(command, check=True)
    print(f"generated {len(variants)} animation state vectors in {ROOT}")


if __name__ == "__main__":
    main()
