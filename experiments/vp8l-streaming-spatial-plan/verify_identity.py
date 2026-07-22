#!/usr/bin/env python3
"""Verify three archive-built encoders against bytes and both decoders."""

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


def sha256(path: Path) -> str:
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


def oracle(
    binary: Path, expected: Path, streams: list[Path], label: str
) -> list[str]:
    result = subprocess.run(
        [str(binary), str(expected), *(str(path) for path in streams)],
        capture_output=True,
        text=True,
        check=True,
    )
    rows = [line for line in result.stdout.splitlines() if line.startswith("oracle\t")]
    if len(rows) != len(LAYOUTS):
        raise RuntimeError(f"{label}: unexpected oracle output: {result.stdout}")
    return [f"{label}\t{line}" for line in rows]


def main() -> None:
    parser = argparse.ArgumentParser()
    for label in ("base", "control", "candidate"):
        parser.add_argument(f"--{label}-binary", type=Path, required=True)
        parser.add_argument(f"--{label}-label", required=True)
    parser.add_argument("--corpus", type=Path, required=True)
    parser.add_argument("--oracle-binary", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    binaries = {label: getattr(args, f"{label}_binary") for label in ("base", "control", "candidate")}
    labels = {label: getattr(args, f"{label}_label") for label in binaries}

    args.output.mkdir(parents=True, exist_ok=False)
    metadata = {
        "binaries": {
            label: {
                "label": labels[label],
                "path": str(path.resolve()),
                "sha256": sha256(path),
            }
            for label, path in binaries.items()
        },
        "corpus": str(args.corpus.resolve()),
        "oracle_binary": str(args.oracle_binary.resolve()),
        "oracle_binary_sha256": sha256(args.oracle_binary),
        "layouts": LAYOUTS,
        "project_decoder": "generate validates exact RGBA before writing every stream",
        "oracle_decoder": "pinned libwebp WebPDecodeRGBA",
    }
    (args.output / "run.json").write_text(json.dumps(metadata, indent=2) + "\n")

    identity_path = args.output / "identity-306.tsv"
    oracle_path = args.output / "oracle-918.tsv"
    with identity_path.open("w") as identity, oracle_path.open("w") as oracle_output:
        identity.write(
            "id\tlayout\tbase_bytes\tcontrol_bytes\tcandidate_bytes\tbase_sha256\t"
            "control_sha256\tcandidate_sha256\tbase_control_length\t"
            "base_control_sha256\tbase_control_full_byte\tbase_candidate_length\t"
            "base_candidate_sha256\tbase_candidate_full_byte\tcontrol_candidate_length\t"
            "control_candidate_sha256\tcontrol_candidate_full_byte\t"
            "base_project_exact\tcontrol_project_exact\tcandidate_project_exact\n"
        )
        oracle_output.write("binary\toracle\tid\twidth\theight\trgba_bytes\n")
        oracle_rows = 0
        with tempfile.TemporaryDirectory(
            prefix="vp8l-streaming-spatial-identity-", dir="/private/tmp"
        ) as temporary:
            root = Path(temporary)
            for index in range(102):
                directories = {label: root / f"{label}-{index}" for label in binaries}
                for label, binary in binaries.items():
                    generate(binary, args.corpus, directories[label], index)
                ids = [path.stem for path in (directories["candidate"] / "default").glob("*.webp")]
                if len(ids) != 1:
                    raise RuntimeError(f"index {index}: generated ids {ids}")
                image = ids[0]
                streams: dict[str, list[Path]] = {label: [] for label in binaries}
                for layout in LAYOUTS:
                    paths = {
                        label: directory / layout / f"{image}.webp"
                        for label, directory in directories.items()
                    }
                    data = {label: path.read_bytes() for label, path in paths.items()}
                    hashes = {label: hashlib.sha256(value).hexdigest() for label, value in data.items()}
                    pairs = (("base", "control"), ("base", "candidate"), ("control", "candidate"))
                    comparisons = []
                    for left, right in pairs:
                        comparisons.extend(
                            (
                                int(len(data[left]) == len(data[right])),
                                int(hashes[left] == hashes[right]),
                                int(data[left] == data[right]),
                            )
                        )
                    if not all(comparisons):
                        raise RuntimeError(f"{image} {layout}: archive stream mismatch")
                    identity.write(
                        "\t".join(
                            [
                                image,
                                layout,
                                *(str(len(data[label])) for label in binaries),
                                *(hashes[label] for label in binaries),
                                *(str(value) for value in comparisons),
                                "1",
                                "1",
                                "1",
                            ]
                        )
                        + "\n"
                    )
                    for label in binaries:
                        streams[label].append(paths[label])
                expected = directories["candidate"] / "expected"
                for label in binaries:
                    rows = oracle(args.oracle_binary, expected, streams[label], label)
                    oracle_output.write("\n".join(rows) + "\n")
                    oracle_rows += len(rows)
                for directory in directories.values():
                    shutil.rmtree(directory)
        oracle_output.write(f"oracle_summary\tmatched={oracle_rows}\tfailed=0\n")
        if oracle_rows != 918:
            raise RuntimeError(f"expected 918 oracle rows, got {oracle_rows}")


if __name__ == "__main__":
    main()
