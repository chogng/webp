#!/usr/bin/env python3
"""Fetch the CLIC validation split and export deterministic PNG inputs."""
import argparse
from pathlib import Path
import hashlib
import json
import zipfile

import tensorflow_datasets as tfds
from PIL import Image

parser = argparse.ArgumentParser()
download = parser.add_mutually_exclusive_group()
download.add_argument(
    "--allow-full-download",
    action="store_true",
    help="download the 7.48 GiB source archive (about 14.96 GiB after TFDS preparation)",
)
parser.add_argument("--mobile-zip", type=Path, help="local CLIC2020 mobile validation zip")
parser.add_argument(
    "--professional-zip", type=Path, help="local CLIC2020 professional validation zip"
)
args = parser.parse_args()
if bool(args.mobile_zip) != bool(args.professional_zip):
    parser.error(
        "--mobile-zip and --professional-zip must be supplied together"
    )
if not args.allow_full_download and not args.mobile_zip:
    parser.error(
        "supply both local validation zip files, or use --allow-full-download for the 7.48 GiB TFDS archive"
    )

root = Path("third_party/benchdata/clic")
images = root / "validation-png"
images.mkdir(parents=True, exist_ok=True)


def sha256_file(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def extract_zip(archive: Path, destination: Path) -> None:
    if not archive.is_file():
        raise SystemExit(f"missing CLIC archive: {archive}")
    destination.mkdir(parents=True, exist_ok=True)
    root_path = destination.resolve()
    with zipfile.ZipFile(archive) as bundle:
        for member in bundle.infolist():
            target = (destination / member.filename).resolve()
            if not target.is_relative_to(root_path):
                raise SystemExit(f"archive entry escapes destination: {member.filename}")
        bundle.extractall(destination)


provenance = {"dataset": "tfds:clic:1.0.0", "split": "validation"}
if args.mobile_zip:
    sources = {
        "mobile": args.mobile_zip,
        "professional": args.professional_zip,
    }
    for label, archive in sources.items():
        extract_zip(archive, images / label)
    source_images = sorted(
        path
        for label in sources
        for path in (images / label).rglob("*.png")
        if "__MACOSX" not in path.parts and not path.name.startswith("._")
    )
    provenance["archives"] = {
        label: {"name": archive.name, "sha256": sha256_file(archive)}
        for label, archive in sources.items()
    }
else:
    dataset = tfds.load("clic", split="validation", data_dir=str(root / "tfds"), download=True)
    source_images = []
    for index, item in enumerate(tfds.as_numpy(dataset)):
        array = item["image"]
        output = images / f"clic-validation-{index:03}.png"
        Image.fromarray(array).save(output)
        source_images.append(output)
    provenance["archives"] = "TFDS full download"

manifest = []
for index, image in enumerate(source_images):
    with Image.open(image) as decoded:
        width, height = decoded.size
        channels = len(decoded.getbands())
    manifest.append({
        "id": f"clic-validation-{index:03}",
        "file": image.relative_to(images).as_posix(),
        "sha256": sha256_file(image),
        "width": width,
        "height": height,
        "channels": channels,
    })

(root / "validation-manifest.json").write_text(
    json.dumps({**provenance, "images": manifest}, indent=2)
    + "\n"
)
print(f"exported {len(manifest)} CLIC validation images to {images}")
