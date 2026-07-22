#!/usr/bin/env python3
"""Summarize and enforce P22's pre-B retained-plan control identity."""

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


def provenance(path: Path) -> dict[str, str]:
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
    prov = provenance(raw / "binary-provenance.txt")
    expected_bytes = {"compact": 599_398_064, "low-latency": 601_400_998}
    profiles: dict[str, object] = {}
    identity = 0
    for profile in ("compact", "low-latency"):
        a = measurements(raw / f"a-{profile}.tsv")
        p18 = measurements(raw / f"p18-{profile}.tsv")
        assert a.keys() == p18.keys()
        mismatches = [image_id for image_id in a if a[image_id] != p18[image_id]]
        assert not mismatches
        identity += len(a)
        aggregate = sum(item[0] for item in a.values())
        assert aggregate == expected_bytes[profile]
        profiles[profile] = {
            "images": len(a),
            "aggregate_bytes": aggregate,
            "p18_size_and_stream_hash_identity": "102/102",
            "aggregate_stream_checksum": typed_rows(raw / f"a-{profile}.tsv", "aggregate")[0][8],
            "raw_sha256": {
                "a": sha256(raw / f"a-{profile}.tsv"),
                "p18": sha256(raw / f"p18-{profile}.tsv"),
            },
        }
    assert identity == 204
    stderr_files = sorted(raw.glob("*.stderr"))
    stderr_bytes = sum(path.stat().st_size for path in stderr_files)
    assert stderr_bytes == 0
    summary = {
        "task": prov["task"],
        "root_task": prov["root_task"],
        "branch": prov["branch"],
        "base": prov["base"],
        "head": prov["head"],
        "worktree": prov["worktree"],
        "a_binary_sha256": prov["a_binary_sha256"],
        "p18_binary_sha256": prov["p18_binary_sha256"],
        "corpus_manifest_sha256": prov["corpus_manifest_sha256"],
        "screen_manifest_sha256": prov["screen_manifest_sha256"],
        "profiles": profiles,
        "p18_size_and_stream_hash_identity": "204/204",
        "stderr_bytes": stderr_bytes,
        "gate": True,
    }
    (output / "a-baseline-summary.json").write_text(json.dumps(summary, indent=2) + "\n")


if __name__ == "__main__":
    main()
