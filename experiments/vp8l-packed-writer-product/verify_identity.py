#!/usr/bin/env python3
"""Stream two encoder binaries through byte and decoder identity checks."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile


TEST = "encoder::product_benchmark_tests::product_validation_reproducer"
LAYOUTS = ("default", "compact", "low-latency")


def file_sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def generate(binary: Path, corpus: Path, output: Path, index: int) -> None:
    environment = os.environ.copy()
    environment.update(
        VP8L_PRODUCT_COMMAND="generate",
        VP8L_PRODUCT_INPUT=str(corpus),
        VP8L_PRODUCT_OUTPUT=str(output),
        VP8L_PRODUCT_START=str(index),
        VP8L_PRODUCT_LIMIT="1",
    )
    subprocess.run(
        [str(binary), "--exact", TEST, "--ignored", "--nocapture"],
        env=environment,
        stdout=subprocess.DEVNULL,
        check=True,
    )


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--left-binary", type=Path, required=True)
    parser.add_argument("--right-binary", type=Path, required=True)
    parser.add_argument("--left-label", required=True)
    parser.add_argument("--right-label", required=True)
    parser.add_argument("--corpus", type=Path, required=True)
    parser.add_argument("--oracle-binary", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()

    args.output.mkdir(parents=True, exist_ok=False)
    metadata = {
        "left_label": args.left_label,
        "left_binary": str(args.left_binary.resolve()),
        "left_binary_sha256": file_sha256(args.left_binary),
        "right_label": args.right_label,
        "right_binary": str(args.right_binary.resolve()),
        "right_binary_sha256": file_sha256(args.right_binary),
        "corpus": str(args.corpus.resolve()),
        "oracle_binary": str(args.oracle_binary.resolve()),
        "oracle_binary_sha256": file_sha256(args.oracle_binary),
        "layouts": LAYOUTS,
        "project_decoder": "generate validates exact RGBA before writing each stream",
        "oracle_decoder": "pinned libwebp WebPDecodeRGBA",
    }
    (args.output / "run.json").write_text(json.dumps(metadata, indent=2) + "\n")

    with (args.output / "identity-306.tsv").open("w") as identity, (
        args.output / "oracle-306.tsv"
    ).open("w") as oracle:
        identity.write(
            "id\tlayout\tleft_label\tright_label\tleft_bytes\tright_bytes\t"
            "left_sha256\tright_sha256\tlength_identity\tsha256_identity\t"
            "full_byte_identity\tleft_project_exact\tright_project_exact\n"
        )
        oracle.write("oracle\tid\twidth\theight\trgba_bytes\n")
        matched = 0
        with tempfile.TemporaryDirectory(
            prefix="vp8l-packed-writer-product-identity-", dir="/private/tmp"
        ) as temporary:
            root = Path(temporary)
            for index in range(102):
                directories = {
                    "left": root / f"left-{index}",
                    "right": root / f"right-{index}",
                }
                generate(args.left_binary, args.corpus, directories["left"], index)
                generate(args.right_binary, args.corpus, directories["right"], index)
                ids = [
                    path.stem
                    for path in (directories["right"] / "default").glob("*.webp")
                ]
                if len(ids) != 1:
                    raise RuntimeError(f"index {index}: generated ids {ids}")
                image = ids[0]
                right_streams = []
                for layout in LAYOUTS:
                    paths = {
                        side: directory / layout / f"{image}.webp"
                        for side, directory in directories.items()
                    }
                    data = {side: path.read_bytes() for side, path in paths.items()}
                    left_sha = hashlib.sha256(data["left"]).hexdigest()
                    right_sha = hashlib.sha256(data["right"]).hexdigest()
                    lengths = len(data["left"]) == len(data["right"])
                    hashes = left_sha == right_sha
                    exact = data["left"] == data["right"]
                    identity.write(
                        f"{image}\t{layout}\t{args.left_label}\t{args.right_label}\t"
                        f"{len(data['left'])}\t{len(data['right'])}\t{left_sha}\t"
                        f"{right_sha}\t{int(lengths)}\t{int(hashes)}\t{int(exact)}\t1\t1\n"
                    )
                    if not lengths or not hashes or not exact:
                        raise RuntimeError(f"{image} {layout}: byte mismatch")
                    right_streams.append(str(paths["right"]))
                result = subprocess.run(
                    [
                        str(args.oracle_binary),
                        str(directories["right"] / "expected"),
                        *right_streams,
                    ],
                    capture_output=True,
                    text=True,
                    check=True,
                )
                lines = [
                    line
                    for line in result.stdout.splitlines()
                    if line.startswith("oracle\t")
                ]
                if len(lines) != len(LAYOUTS):
                    raise RuntimeError(f"{image}: oracle output {result.stdout}")
                for line in lines:
                    oracle.write(line + "\n")
                matched += len(lines)
                for directory in directories.values():
                    shutil.rmtree(directory)
        oracle.write(f"oracle_summary\tmatched={matched}\tfailed=0\n")


if __name__ == "__main__":
    main()
