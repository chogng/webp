#!/usr/bin/env python3
"""Summarize the final locked packed-writer product benchmarks."""

from __future__ import annotations

import csv
import json
from pathlib import Path
import statistics
import sys


def median(values: list[float]) -> float:
    return statistics.median(values)


def mad(values: list[float]) -> float:
    center = median(values)
    return median([abs(value - center) for value in values])


def percentiles(values: list[float]) -> dict[str, float]:
    ordered = sorted(values)
    deciles = statistics.quantiles(ordered, n=10, method="inclusive")
    return {
        "p0": ordered[0],
        "p10": deciles[0],
        "p50": median(ordered),
        "p90": deciles[8],
        "p100": ordered[-1],
    }


def measurements(root: Path) -> tuple[dict[str, list[float]], dict[str, dict[str, list[int]]]]:
    aggregates: dict[str, list[tuple[int, float]]] = {}
    images: dict[str, dict[str, list[int]]] = {}
    for path in (root / "measurements").glob("*.tsv"):
        for row in csv.reader(path.open(), delimiter="\t"):
            if not row or len(row) < 6 or row[2] == "warmup":
                continue
            if row[0] == "aggregate":
                aggregates.setdefault(row[3], []).append((int(row[2]), int(row[5]) / 1e9))
            elif row[0] == "measurement":
                images.setdefault(row[3], {}).setdefault(row[4], []).append(int(row[5]))
    return (
        {layout: [value for _, value in sorted(rows)] for layout, rows in aggregates.items()},
        images,
    )


def pair(root: Path, baseline: str, candidate: str) -> dict[str, object]:
    aggregates, images = measurements(root)
    base = aggregates[baseline]
    cand = aggregates[candidate]
    base_median = median(base)
    candidate_median = median(cand)
    paired = [100 * (right / left - 1) for left, right in zip(base, cand)]
    per_image = [
        100 * (median(images[candidate][image]) / median(images[baseline][image]) - 1)
        for image in images[baseline]
    ]
    process_rows = [
        json.loads(line) for line in (root / "processes.jsonl").read_text().splitlines()
    ]

    def resources(layout: str) -> dict[str, float]:
        rows = [
            row
            for row in process_rows
            if row.get("phase") == "formal" and row["layout"] == layout
        ]
        return {
            "process_wall_seconds_median": median(
                [row["process_wall_ns"] / 1e9 for row in rows]
            ),
            "cpu_seconds_median": median(
                [(row["user_ns"] + row["sys_ns"]) / 1e9 for row in rows]
            ),
            "rss_mib_median": median(
                [row["max_rss_bytes"] / 1024 / 1024 for row in rows]
            ),
        }

    candidate_mad = mad(cand)
    return {
        "baseline_round_seconds": base,
        "candidate_round_seconds": cand,
        "baseline_median_seconds": base_median,
        "baseline_mad_seconds": mad(base),
        "candidate_median_seconds": candidate_median,
        "candidate_mad_seconds": candidate_mad,
        "independent_change_percent": 100 * (candidate_median / base_median - 1),
        "paired_change_percent": paired,
        "paired_median_percent": median(paired),
        "candidate_3mad_rounds": [
            index + 1
            for index, value in enumerate(cand)
            if candidate_mad == 0 or abs(value - candidate_median) > 3 * candidate_mad
        ],
        "image_change_percentiles": percentiles(per_image),
        "image_regressions": sum(value > 0 for value in per_image),
        "baseline_resources": resources(baseline),
        "candidate_resources": resources(candidate),
    }


def main() -> None:
    root = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(__file__).parent
    summary = {
        stage: {
            profile: pair(
                root / "raw" / directory,
                f"{profile}-writer-control",
                profile,
            )
            for profile in ("compact", "low-latency")
        }
        for stage, directory in (
            ("screen", "screen-41-final"),
            ("formal", "formal-102-final"),
        )
    }
    (root / "gate-summary.json").write_text(json.dumps(summary, indent=2) + "\n")


if __name__ == "__main__":
    main()
