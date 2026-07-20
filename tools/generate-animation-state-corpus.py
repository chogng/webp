#!/usr/bin/env python3
"""Generate animation state vectors with explicit mux frame controls."""
from __future__ import annotations

from hashlib import sha256
import json
from pathlib import Path
import subprocess


ROOT = Path("third_party/corpus/animation-v1")
ORACLE = Path("third_party/oracle/libwebp")
WIDTH, HEIGHT = 128, 96


def write_ppm(path: Path, width: int, height: int, color: tuple[int, int, int]) -> None:
    path.write_bytes(f"P6\n{width} {height}\n255\n".encode() + bytes(color) * (width * height))


def toml_string(value: str) -> str:
    return json.dumps(value)


def main() -> None:
    cwebp = ORACLE / "build/cwebp"
    webpmux = ORACLE / "build/webpmux"
    if not (ORACLE / ".git").is_dir() or not cwebp.is_file() or not webpmux.is_file():
        raise SystemExit("build cwebp and webpmux in third_party/oracle/libwebp first")
    revision = subprocess.check_output(["git", "-C", str(ORACLE), "rev-parse", "HEAD"], text=True).strip()
    inputs, frames, manifests = (ROOT / name for name in ("state-inputs", "state-frames", "manifests"))
    for directory in (inputs, frames, manifests):
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
            "notes": "Opaque base followed by an offset blend frame; loop count is one.",
        },
        "animation-dispose-no-blend-loop-zero": {
            "frames": [
                ("base", 100, 0, 0, 0, "+b"),
                ("patch", 40, 32, 24, 1, "-b"),
                ("final", 80, 48, 24, 0, "-b"),
            ],
            "loop": 0,
            "background": "255,10,20,30",
            "notes": "Offset patch disposes to background before the final no-blend frame; loop is infinite.",
        },
    }

    for name, variant in variants.items():
        output = ROOT / f"{name}.webp"
        command = [webpmux]
        frame_args = []
        for frame, duration, x, y, dispose, blend in variant["frames"]:
            controls = f"+{duration}+{x}+{y}+{dispose}{blend}"
            command.extend(["-frame", encoded[frame], controls])
            frame_args.extend(["-frame", f"{frame}.webp", controls])
        command.extend(["-loop", str(variant["loop"]), "-bgcolor", variant["background"], "-o", output])
        subprocess.run(command, check=True)
        contents = [
            f"id = {toml_string(f'oracle-{name}')}",
            f"file = {toml_string(f'../{output.name}')}",
            f"sha256 = {toml_string(sha256(output.read_bytes()).hexdigest())}",
            'class = "MustAccept"',
            'source = "libwebp main webpmux animation state matrix"',
            'license = "BSD-3-Clause"',
            'codec = "Mixed"',
            'api = "ReadInfo"',
            'features = ["animation", "state-matrix", "blend-dispose-loop"]',
            f"expected_width = {WIDTH}",
            f"expected_height = {HEIGHT}",
            f"oracle_revision = {toml_string(revision)}",
            "generator_args = [" + ", ".join(toml_string(argument) for argument in ["webpmux", *frame_args, "-loop", str(variant["loop"]), "-bgcolor", variant["background"]]) + "]",
            f"notes = {toml_string(variant['notes'])}",
            "",
        ]
        (manifests / f"{name}.toml").write_text("\n".join(contents))
    print(f"generated {len(variants)} animation state vectors in {ROOT} ({revision})")


if __name__ == "__main__":
    main()
