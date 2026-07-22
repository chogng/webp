#!/usr/bin/env python3
"""Validate P25's one-shot 41-image F/R/F recovery screen."""

from __future__ import annotations

import csv
import json
import statistics
import sys
from pathlib import Path


def samples(raw: Path, layout: str, side: str) -> dict[str, list[tuple[int, int, str]]]:
    result: dict[str, list[tuple[int, int, str]]] = {}
    for label in ("forward-1", "reverse-2", "forward-3"):
        path = raw / f"{layout}-{label}-{side}.tsv"
        assert path.is_file()
        with path.open(newline="") as stream:
            rows = [row for row in csv.reader(stream, delimiter="\t") if row and row[0] == "measurement"]
        assert len(rows) == 41
        for row in rows:
            result.setdefault(row[4], []).append((int(row[5]), int(row[7]), row[8]))
    assert len(result) == 41
    assert all(len(values) == 3 for values in result.values())
    return result


def warmup(raw: Path, layout: str, side: str) -> set[str]:
    path = raw / f"{layout}-warmup-{side}.tsv"
    assert path.is_file()
    with path.open(newline="") as stream:
        rows = [row for row in csv.reader(stream, delimiter="\t") if row and row[0] == "measurement"]
    assert len(rows) == 41
    return {row[4] for row in rows}


def main() -> None:
    output = Path(sys.argv[1]).resolve()
    raw = output / "raw"
    provenance = dict(line.split("=", 1) for line in (raw / "provenance.txt").read_text().splitlines())
    profiles = {"compact": "compact-fused-rank-sum", "low-latency": "low-latency-fused-rank-sum"}
    warmup_ids = {
        (layout, side): warmup(raw, layout, side)
        for layout, side in [
            ("compact", "a"),
            ("compact-fused-rank-sum", "b"),
            ("low-latency", "a"),
            ("low-latency-fused-rank-sum", "b"),
        ]
    }
    assert len(warmup_ids) == 4
    assert len({frozenset(ids) for ids in warmup_ids.values()}) == 1
    summary = {}
    gate = True
    for control, candidate in profiles.items():
        a = samples(raw, control, "a")
        b = samples(raw, candidate, "b")
        assert a.keys() == b.keys()
        assert all(len({item[1:] for item in a[key] + b[key]}) == 1 for key in a)
        a_total = [sum(values[index][0] for values in a.values()) for index in range(3)]
        b_total = [sum(values[index][0] for values in b.values()) for index in range(3)]
        a_median = statistics.median(a_total)
        b_median = statistics.median(b_total)
        per_image_deltas = {
            image_id: (statistics.median(value[0] for value in b[image_id]) - statistics.median(value[0] for value in a[image_id])) * 100
            / statistics.median(value[0] for value in a[image_id])
            for image_id in a
        }
        regressions = [
            {"id": image_id, "delta_percent": per_image_deltas[image_id]}
            for image_id in sorted(per_image_deltas)
            if per_image_deltas[image_id] > 0
        ]
        improvement = (a_median - b_median) * 100 / a_median
        profile_gate = (
            improvement >= 5.0 and not regressions
            if control == "low-latency"
            else improvement >= -0.5
        )
        gate = gate and profile_gate
        summary[control] = {
            "a_samples_ns": a_total,
            "b_samples_ns": b_total,
            "a_median_ns": a_median,
            "b_median_ns": b_median,
            "improvement_percent": improvement,
            "per_image_regressions": regressions,
            "gate": profile_gate,
            "identity": "41/41",
        }
    stderr_bytes = sum(path.stat().st_size for path in raw.glob("*.stderr"))
    assert stderr_bytes == 0
    (output / "recovery-summary.json").write_text(json.dumps({
        "task": "P25",
        "binary_source_head": provenance["binary_source_head"],
        "runner_head": provenance["runner_head"],
        "p25_binary_sha256": provenance["p25_binary_sha256"],
        "screen_manifest_sha256": provenance["screen_manifest_sha256"],
        "profiles": summary,
        "stderr_bytes": stderr_bytes,
        "identity": "82/82",
        "gate": gate,
    }, indent=2) + "\n")
    if not gate:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
