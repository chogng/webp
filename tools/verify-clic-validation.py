#!/usr/bin/env python3
"""Verify the local CLIC PNG benchmark corpus without needing its source zips."""
from pathlib import Path
import hashlib
import json

from PIL import Image

root = Path("third_party/benchdata/clic")
images = root / "validation-png"
manifest_path = root / "validation-manifest.json"
manifest = json.loads(manifest_path.read_text())

if manifest.get("split") != "validation" or not manifest.get("images"):
    raise SystemExit("invalid or empty CLIC validation manifest")

for entry in manifest["images"]:
    relative = Path(entry["file"])
    if relative.is_absolute():
        raise SystemExit(f"absolute image path in manifest: {relative}")
    image = (images / relative).resolve()
    if not image.is_relative_to(images.resolve()):
        raise SystemExit(f"image path escapes corpus: {relative}")
    actual_hash = hashlib.sha256(image.read_bytes()).hexdigest()
    if actual_hash != entry["sha256"]:
        raise SystemExit(f"sha256 mismatch: {relative}")
    with Image.open(image) as decoded:
        width, height = decoded.size
        channels = len(decoded.getbands())
    if (width, height, channels) != (entry["width"], entry["height"], entry["channels"]):
        raise SystemExit(f"geometry mismatch: {relative}")

print(f"verified {len(manifest['images'])} CLIC validation images")
