#!/usr/bin/env python3
"""Stream latest-main/product/P09 identity and pinned-libwebp RGBA checks."""

from __future__ import annotations

import argparse
import hashlib
import os
from pathlib import Path
import shutil
import subprocess
import tempfile


TEST = "encoder::product_benchmark_tests::product_validation_reproducer"
LAYOUTS = ("default", "compact", "low-latency")


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


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--base-binary", type=Path, required=True)
    parser.add_argument("--product-binary", type=Path, required=True)
    parser.add_argument("--p09-binary", type=Path, required=True)
    parser.add_argument("--corpus", type=Path, required=True)
    parser.add_argument("--oracle-binary", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=False)
    identity_path = args.output / "identity-306.tsv"
    oracle_path = args.output / "oracle-306.tsv"
    with identity_path.open("w") as identity, oracle_path.open("w") as oracle:
        identity.write(
            "id\tlayout\tbase_bytes\tproduct_bytes\tp09_bytes\tbase_sha256\t"
            "product_sha256\tp09_sha256\tbase_product_identity\t"
            "product_p09_identity\tproject_exact\n"
        )
        oracle.write("oracle\tid\twidth\theight\trgba_bytes\n")
        matched = 0
        with tempfile.TemporaryDirectory(
            prefix="vp8l-exact-cost-product-identity-", dir="/private/tmp"
        ) as temporary:
            root = Path(temporary)
            for index in range(102):
                directories = {
                    name: root / f"{name}-{index}"
                    for name in ("base", "product", "p09")
                }
                generate(args.base_binary, args.corpus, directories["base"], index)
                generate(args.product_binary, args.corpus, directories["product"], index)
                generate(args.p09_binary, args.corpus, directories["p09"], index)
                ids = [
                    path.stem
                    for path in (directories["product"] / "default").glob("*.webp")
                ]
                if len(ids) != 1:
                    raise RuntimeError(f"index {index}: generated ids {ids}")
                image = ids[0]
                streams = []
                for layout in LAYOUTS:
                    paths = {
                        name: directory / layout / f"{image}.webp"
                        for name, directory in directories.items()
                    }
                    data = {name: path.read_bytes() for name, path in paths.items()}
                    base_product = data["base"] == data["product"]
                    product_p09 = data["product"] == data["p09"]
                    identity.write(
                        f"{image}\t{layout}\t{len(data['base'])}\t"
                        f"{len(data['product'])}\t{len(data['p09'])}\t"
                        f"{sha256(data['base'])}\t{sha256(data['product'])}\t"
                        f"{sha256(data['p09'])}\t{int(base_product)}\t"
                        f"{int(product_p09)}\t1\n"
                    )
                    if not base_product or not product_p09:
                        raise RuntimeError(f"{image} {layout}: byte mismatch")
                    streams.append(str(paths["product"]))
                result = subprocess.run(
                    [
                        str(args.oracle_binary),
                        str(directories["product"] / "expected"),
                        *streams,
                    ],
                    capture_output=True,
                    text=True,
                    check=True,
                )
                lines = [
                    line for line in result.stdout.splitlines() if line.startswith("oracle\t")
                ]
                if len(lines) != 3:
                    raise RuntimeError(f"{image}: oracle output {result.stdout}")
                for line in lines:
                    oracle.write(line + "\n")
                matched += len(lines)
                for directory in directories.values():
                    shutil.rmtree(directory)
        oracle.write(f"oracle_summary\tmatched={matched}\tfailed=0\n")


if __name__ == "__main__":
    main()
