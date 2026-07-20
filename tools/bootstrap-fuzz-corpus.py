#!/usr/bin/env python3
"""Materialize deterministic local seed corpora for the Rust fuzz targets."""

from __future__ import annotations

import argparse
import shutil
from pathlib import Path


REPOSITORY = Path(__file__).resolve().parents[1]
FIXTURES = REPOSITORY / "tests" / "fixtures"


def write_if_changed(path: Path, data: bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    if not path.is_file() or path.read_bytes() != data:
        path.write_bytes(data)


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

    fixture_files = sorted((FIXTURES / "generated").glob("*.webp"))
    fixture_files += sorted((FIXTURES / "smoke").glob("*.webp"))
    if not fixture_files:
        raise SystemExit("no committed WebP fixtures found; run from a complete checkout")

    targets = ("container_raw", "incremental_raw", "vp8l_header_raw")
    for target in targets:
        for fixture in fixture_files:
            write_if_changed(output / target / fixture.name, fixture.read_bytes())

    raw_seeds = {
        "header-only": bytes((0x2F, 0, 0, 0, 0)),
        "signature-only": bytes((0x2F,)),
        "literal-1x1": literal_vp8l(),
    }
    for name, payload in raw_seeds.items():
        write_if_changed(output / "vp8l_raw" / name, payload)
    huffman_seeds = {
        "alphabet-1-empty-code": bytes((0, 0, 1, 0, 0)),
        "alphabet-256-simple-code": bytes((255, 0, 1, 0, 0)),
        "max-alphabet-normal-code": bytes((0x17, 0x09, 0, 0, 0, 0)),
    }
    for name, payload in huffman_seeds.items():
        write_if_changed(output / "vp8l_huffman" / name, payload)
    transform_seeds = {
        "one-pixel": bytes((0, 0, 0, 0, 0, 255)),
        "max-bounded-shape": bytes(range(256)),
    }
    for name, payload in transform_seeds.items():
        write_if_changed(output / "vp8l_transforms" / name, payload)
    write_if_changed(
        output / "vp8l_header_raw" / "valid-vp8l-header.webp",
        vp8l_riff(raw_seeds["header-only"]),
    )

    print(
        f"seeded {len(fixture_files)} fixture inputs for {len(targets)} targets and "
        f"{len(raw_seeds)} raw VP8L, {len(huffman_seeds)} Huffman, and "
        f"{len(transform_seeds)} transform seeds under {output}"
    )


if __name__ == "__main__":
    main()
