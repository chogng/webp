#!/usr/bin/env python3
"""Fetch the CLIC validation split and export deterministic PNG inputs."""
import argparse
from pathlib import Path
import hashlib
import json

import tensorflow_datasets as tfds
from PIL import Image

parser = argparse.ArgumentParser()
parser.add_argument(
    "--allow-full-download",
    action="store_true",
    help="download the 7.48 GiB source archive (about 14.96 GiB after TFDS preparation)",
)
args = parser.parse_args()
if not args.allow_full_download:
    parser.error(
        "CLIC validation requires a 7.48 GiB download and about 14.96 GiB of workspace; "
        "rerun with --allow-full-download"
    )

root = Path("third_party/benchdata/clic")
images = root / "validation-png"
images.mkdir(parents=True, exist_ok=True)

dataset = tfds.load("clic", split="validation", data_dir=str(root / "tfds"), download=True)
manifest = []
for index, item in enumerate(tfds.as_numpy(dataset)):
    array = item["image"]
    output = images / f"clic-validation-{index:03}.png"
    Image.fromarray(array).save(output)
    manifest.append({
        "id": output.stem,
        "file": output.name,
        "sha256": hashlib.sha256(output.read_bytes()).hexdigest(),
        "width": int(array.shape[1]),
        "height": int(array.shape[0]),
        "channels": int(array.shape[2]) if array.ndim == 3 else 1,
    })

(root / "validation-manifest.json").write_text(
    json.dumps({"dataset": "tfds:clic:1.0.0", "split": "validation", "images": manifest}, indent=2)
    + "\n"
)
print(f"exported {len(manifest)} CLIC validation images to {images}")
