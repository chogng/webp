#!/usr/bin/env python3
"""Materialize deterministic local seed corpora for the Rust fuzz targets."""

from __future__ import annotations

import argparse
import hashlib
import os
import tempfile
from pathlib import Path


REPOSITORY = Path(__file__).resolve().parents[1]
FIXTURES = REPOSITORY / "tests" / "fixtures"
FIXTURE_MANIFEST_HEADER = "webp-fixture-manifest-v1"
CURRENT_PREFIX = "CURRENT-"
BOOTSTRAP_MANIFEST = ".bootstrap-owned"


def write_if_changed(path: Path, data: bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    if path.is_file() and path.read_bytes() == data:
        return
    temporary: Path | None = None
    try:
        with tempfile.NamedTemporaryFile(
            dir=path.parent, prefix=f".{path.name}.", delete=False
        ) as handle:
            temporary = Path(handle.name)
            handle.write(data)
            handle.flush()
            os.fsync(handle.fileno())
        temporary.replace(path)
    finally:
        if temporary is not None:
            temporary.unlink(missing_ok=True)


def current_generated_fixtures() -> list[Path]:
    root = FIXTURES / "generated"
    markers: list[tuple[int, str, Path]] = []
    for marker in root.glob(f"{CURRENT_PREFIX}*"):
        if not marker.is_file():
            continue
        fields = marker.name.removeprefix(CURRENT_PREFIX).split("-", 1)
        if (
            len(fields) != 2
            or len(fields[0]) != 20
            or not fields[0].isdigit()
            or len(fields[1]) != 64
            or any(character not in "0123456789abcdefABCDEF" for character in fields[1])
        ):
            continue
        markers.append((int(fields[0]), fields[1].lower(), marker))
    if not markers:
        raise SystemExit(
            "fixture cache has no committed generation; "
            "run `cd webp-rs && cargo run -p xtask -- fixtures ensure`"
        )

    _, digest, marker = max(markers)
    if marker.read_text() != f"{digest}\n":
        raise SystemExit(f"malformed fixture cache marker: {marker}")
    generation = root / "sets" / digest
    manifest = generation / "MANIFEST.sha256"
    lines = manifest.read_text().splitlines()
    if not lines or lines[0] != FIXTURE_MANIFEST_HEADER:
        raise SystemExit(f"unsupported fixture manifest: {manifest}")

    fixtures: list[Path] = []
    names: set[str] = set()
    for line in lines[1:]:
        fields = line.split(" ", 2)
        if len(fields) != 3:
            raise SystemExit(f"malformed fixture manifest row: {line!r}")
        expected_hash, size_text, name = fields
        if (
            len(expected_hash) != 64
            or any(character not in "0123456789abcdef" for character in expected_hash)
            or not size_text.isdigit()
            or not name.endswith(".webp")
            or Path(name).name != name
            or any(character.isspace() for character in name)
            or name in names
        ):
            raise SystemExit(f"invalid fixture manifest row: {line!r}")
        path = generation / name
        data = path.read_bytes()
        if len(data) != int(size_text) or hashlib.sha256(data).hexdigest() != expected_hash:
            raise SystemExit(f"fixture integrity check failed: {path}")
        names.add(name)
        fixtures.append(path)

    actual_names = {path.name for path in generation.glob("*.webp") if path.is_file()}
    if actual_names != names or not fixtures:
        raise SystemExit(f"incomplete fixture generation: {generation}")
    return fixtures


def write_owned(output: Path, relative: Path, data: bytes, owned: set[str]) -> None:
    write_if_changed(output / relative, data)
    owned.add(relative.as_posix())


def publish_owned_manifest(output: Path, owned: set[str]) -> None:
    manifest = output / BOOTSTRAP_MANIFEST
    previous = set()
    if manifest.is_file():
        lines = manifest.read_text().splitlines()
        if lines and lines[0] == "webp-fuzz-bootstrap-v1":
            lines = lines[1:]
        for line in lines:
            relative = Path(line)
            if line and not relative.is_absolute() and ".." not in relative.parts:
                previous.add(relative.as_posix())
    for stale in sorted(previous - owned):
        path = output / stale
        if path.is_file():
            path.unlink()
    write_if_changed(
        manifest,
        ("webp-fuzz-bootstrap-v1\n" + "".join(f"{path}\n" for path in sorted(owned))).encode(),
    )


def literal_vp8l() -> bytes:
    """The committed 1x1 literal-only VP8L stream used by the public API test."""
    bits: list[int] = []

    def push(value: int, width: int) -> None:
        bits.extend((value >> bit) & 1 for bit in range(width))

    push(0x2F, 8)
    push(0, 14)
    push(0, 14)
    push(1, 1)
    push(0, 3)
    push(0, 3)
    for channel in (0x34, 0x12, 0x56, 0x78, 0):
        push(1, 1)
        push(0, 1)
        push(1, 1)
        push(channel, 8)
    return bytes(
        sum(bits[index + bit] << bit for bit in range(min(8, len(bits) - index)))
        for index in range(0, len(bits), 8)
    )


def vp8l_riff(payload: bytes) -> bytes:
    padding = b"\0" if len(payload) % 2 else b""
    body = b"WEBPVP8L" + len(payload).to_bytes(4, "little") + payload + padding
    return b"RIFF" + len(body).to_bytes(4, "little") + body


def chunk(fourcc: bytes, payload: bytes) -> bytes:
    padding = b"\0" if len(payload) % 2 else b""
    return fourcc + len(payload).to_bytes(4, "little") + payload + padding


def minimal_animation() -> bytes:
    """A valid 1x1 ANIM/ANMF WebP that reaches canvas composition."""
    vp8x = bytes((0x02, 0, 0, 0, 0, 0, 0, 0, 0, 0))
    anim = bytes(6)
    frame_header = bytes(12) + bytes((1, 0, 0, 0))
    frame = frame_header + chunk(b"VP8L", literal_vp8l())
    body = b"WEBP" + chunk(b"VP8X", vp8x) + chunk(b"ANIM", anim) + chunk(b"ANMF", frame)
    return b"RIFF" + len(body).to_bytes(4, "little") + body


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--output",
        type=Path,
        default=REPOSITORY / "fuzz" / "corpus",
        help="ignored corpus root to populate (default: fuzz/corpus)",
    )
    args = parser.parse_args()
    output = args.output.resolve()

    fixture_files = current_generated_fixtures()
    fixture_files += sorted((FIXTURES / "smoke").glob("*.webp"))
    if not fixture_files:
        raise SystemExit("no verified WebP fixtures found")

    owned: set[str] = set()
    targets = ("container_raw", "incremental_raw", "vp8l_header_raw")
    for target in targets:
        for fixture in fixture_files:
            write_owned(output, Path(target) / fixture.name, fixture.read_bytes(), owned)

    raw_seeds = {
        "header-only": bytes((0x2F, 0, 0, 0, 0)),
        "signature-only": bytes((0x2F,)),
        "literal-1x1": literal_vp8l(),
    }
    for name, payload in raw_seeds.items():
        write_owned(output, Path("vp8l_raw") / name, payload, owned)
    huffman_seeds = {
        "alphabet-1-empty-code": bytes((0, 0, 1, 0, 0)),
        "alphabet-256-simple-code": bytes((255, 0, 1, 0, 0)),
        "max-alphabet-normal-code": bytes((0x17, 0x09, 0, 0, 0, 0)),
    }
    for name, payload in huffman_seeds.items():
        write_owned(output, Path("vp8l_huffman") / name, payload, owned)
    transform_seeds = {
        "one-pixel": bytes((0, 0, 0, 0, 0, 255)),
        "max-bounded-shape": bytes(range(256)),
    }
    for name, payload in transform_seeds.items():
        write_owned(output, Path("vp8l_transforms") / name, payload, owned)
    write_owned(
        output,
        Path("vp8l_header_raw") / "valid-vp8l-header.webp",
        vp8l_riff(raw_seeds["header-only"]),
        owned,
    )
    write_owned(
        output,
        Path("animation_raw") / "minimal-animation.webp",
        minimal_animation(),
        owned,
    )
    external_animations = REPOSITORY / "third_party" / "corpus" / "animation-v1"
    if external_animations.is_dir():
        for animation in sorted(external_animations.glob("*.webp")):
            write_owned(
                output,
                Path("animation_raw") / animation.name,
                animation.read_bytes(),
                owned,
            )
    publish_owned_manifest(output, owned)

    print(
        f"seeded {len(fixture_files)} fixture inputs for {len(targets)} targets and "
        f"{len(raw_seeds)} raw VP8L, {len(huffman_seeds)} Huffman, and "
        f"{len(transform_seeds)} transform seeds plus animation seeds under {output}"
    )


if __name__ == "__main__":
    main()
