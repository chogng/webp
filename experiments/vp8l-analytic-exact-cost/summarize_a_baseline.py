#!/usr/bin/env python3
"""Enforce P23's pre-B retained-full-plan control identity."""

from __future__ import annotations

import csv
import hashlib
import json
import sys
from pathlib import Path


def typed_rows(path: Path, kind: str) -> list[list[str]]:
    with path.open(newline="") as stream:
        return [row for row in csv.reader(stream, delimiter="\t") if row and row[0] == kind]


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def read_provenance(path: Path) -> dict[str, str]:
    return dict(line.split("=", 1) for line in path.read_text().splitlines())


def measurements(path: Path) -> dict[str, tuple[int, str]]:
    result: dict[str, tuple[int, str]] = {}
    for row in typed_rows(path, "measurement"):
        image_id = row[4]
        assert image_id not in result
        result[image_id] = (int(row[7]), row[8])
    assert len(result) == 102
    return result


def main() -> None:
    output = Path(sys.argv[1]).resolve()
    raw = output / "raw"
    provenance = read_provenance(raw / "binary-provenance.txt")
    expected_bytes = {"compact": 599_398_064, "low-latency": 601_400_998}
    profiles: dict[str, object] = {}
    identity = 0
    for profile in ("compact", "low-latency"):
        control = measurements(raw / f"a-{profile}.tsv")
        oracle = measurements(raw / f"p18-{profile}.tsv")
        assert control.keys() == oracle.keys()
        assert not [image_id for image_id in control if control[image_id] != oracle[image_id]]
        identity += len(control)
        aggregate = sum(item[0] for item in control.values())
        assert aggregate == expected_bytes[profile]
        profiles[profile] = {
            "images": len(control),
            "aggregate_bytes": aggregate,
            "p18_size_and_stream_hash_identity": "102/102",
            "aggregate_stream_checksum": typed_rows(
                raw / f"a-{profile}.tsv", "aggregate"
            )[0][8],
            "raw_sha256": {
                "a": sha256(raw / f"a-{profile}.tsv"),
                "p18": sha256(raw / f"p18-{profile}.tsv"),
            },
        }
    assert identity == 204
    stderr_bytes = sum(path.stat().st_size for path in raw.glob("*.stderr"))
    assert stderr_bytes == 0
    summary = {
        "task": provenance["task"],
        "root_task": provenance["root_task"],
        "branch": provenance["branch"],
        "base": provenance["base"],
        "head": provenance["head"],
        "worktree": provenance["worktree"],
        "a_binary_sha256": provenance["a_binary_sha256"],
        "p18_binary_sha256": provenance["p18_binary_sha256"],
        "corpus_manifest_sha256": provenance["corpus_manifest_sha256"],
        "screen_manifest_sha256": provenance["screen_manifest_sha256"],
        "profiles": profiles,
        "p18_size_and_stream_hash_identity": "204/204",
        "stderr_bytes": stderr_bytes,
        "gate": True,
    }
    (output / "a-baseline-summary.json").write_text(json.dumps(summary, indent=2) + "\n")


if __name__ == "__main__":
    main()
