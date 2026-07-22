#!/usr/bin/env python3
"""Summarize all retained P13 attribution, screening, and validation evidence."""

from __future__ import annotations

import csv
import json
from pathlib import Path
import statistics
import sys


def robust(values: list[float]) -> dict[str, object]:
    center = statistics.median(values)
    mad = statistics.median(abs(value - center) for value in values)
    return {
        "values": values,
        "median": center,
        "mad": mad,
        "three_mad_outlier_rounds": [
            index + 1
            for index, value in enumerate(values)
            if mad != 0 and abs(value - center) > 3 * mad
        ],
    }


def percentile(values: list[float], fraction: float) -> float:
    ordered = sorted(values)
    position = fraction * (len(ordered) - 1)
    lower = int(position)
    upper = min(lower + 1, len(ordered) - 1)
    weight = position - lower
    return ordered[lower] * (1 - weight) + ordered[upper] * weight


def measurements(root: Path) -> tuple[dict, dict]:
    aggregates: dict[str, dict[int, tuple[int, int, str]]] = {}
    images: dict[str, dict[str, dict[int, tuple[int, int, str]]]] = {}
    for path in sorted((root / "measurements").glob("*.tsv")):
        for row in csv.reader(path.open(), delimiter="\t"):
            if len(row) < 6 or row[2] == "warmup":
                continue
            if row[0] == "aggregate" and row[1] == "encode":
                aggregates.setdefault(row[3], {})[int(row[2])] = (
                    int(row[5]), int(row[7]), row[8]
                )
            elif row[0] == "measurement" and row[1] == "encode":
                images.setdefault(row[3], {}).setdefault(row[4], {})[
                    int(row[2])
                ] = (int(row[5]), int(row[7]), row[8])
    return aggregates, images


def pair(root: Path, control: str, candidate: str) -> dict[str, object]:
    aggregates, images = measurements(root)
    rounds = [1, 2, 3]
    for layout in (control, candidate):
        if sorted(aggregates.get(layout, {})) != rounds:
            raise RuntimeError(f"{root}: incomplete {layout} aggregates")
        if len(images.get(layout, {})) != 41:
            raise RuntimeError(f"{root}: incomplete {layout} images")
    control_wall = [aggregates[control][round_][0] for round_ in rounds]
    candidate_wall = [aggregates[candidate][round_][0] for round_ in rounds]
    for round_ in rounds:
        if aggregates[control][round_][1:] != aggregates[candidate][round_][1:]:
            raise RuntimeError(f"{root}: aggregate output mismatch in round {round_}")
    image_changes = []
    for image in sorted(images[control]):
        for round_ in rounds:
            if images[control][image][round_][1:] != images[candidate][image][round_][1:]:
                raise RuntimeError(f"{root}: {image} output mismatch in round {round_}")
        before = statistics.median(images[control][image][round_][0] for round_ in rounds)
        after = statistics.median(images[candidate][image][round_][0] for round_ in rounds)
        image_changes.append(100 * (after / before - 1))
    processes = [json.loads(line) for line in (root / "processes.jsonl").read_text().splitlines()]

    def resources(layout: str) -> dict[str, object]:
        rows = sorted(
            (row for row in processes if row.get("phase") == "formal" and row["layout"] == layout),
            key=lambda row: row["round"],
        )
        rss = [int(row["max_rss_bytes"]) for row in rows]
        return {
            "process_wall_ns": robust([int(row["process_wall_ns"]) for row in rows]),
            "cpu_ns": robust([int(row["user_ns"]) + int(row["sys_ns"]) for row in rows]),
            "max_rss_bytes": robust(rss),
            "maximum_process_peak_bytes": max(rss),
        }

    control_stats = robust(control_wall)
    candidate_stats = robust(candidate_wall)
    paired = [100 * (after / before - 1) for before, after in zip(control_wall, candidate_wall)]
    control_resources = resources(control)
    candidate_resources = resources(candidate)
    rss_delta = (
        candidate_resources["maximum_process_peak_bytes"]
        - control_resources["maximum_process_peak_bytes"]
    )
    rss_percent = 100 * rss_delta / control_resources["maximum_process_peak_bytes"]
    independent = 100 * (candidate_stats["median"] / control_stats["median"] - 1)
    regressions = sum(value > 0 for value in image_changes)
    return {
        "control": control,
        "candidate": candidate,
        "control_wall_ns": control_stats,
        "candidate_wall_ns": candidate_stats,
        "independent_change_percent": independent,
        "paired_change_percent": robust(paired),
        "per_image_change_percentiles": {
            label: percentile(image_changes, fraction)
            for label, fraction in (("p0", 0), ("p10", .1), ("p50", .5), ("p90", .9), ("p100", 1))
        },
        "image_regressions": regressions,
        "output_identity": True,
        "control_resources": control_resources,
        "candidate_resources": candidate_resources,
        "rss_peak_delta_bytes": rss_delta,
        "rss_peak_delta_percent": rss_percent,
        "rss_gate": rss_delta <= 64 * 1024 * 1024 and rss_percent <= 5,
        "screen_gate": independent <= -10 and regressions == 0,
    }


def phase_a(root: Path) -> dict[str, object]:
    path = root / "raw" / "phase-a-102" / "phase-a.tsv"
    rows = []
    for row in csv.DictReader(
        (line for line in path.open() if line.startswith("phase_a\t")), delimiter="\t"
    ):
        if row["id"] == "id":
            continue
        rows.append(row)
    phases = (
        "residual_ns", "tokenization_ns", "block_census_ns", "seed_rank_assign_ns",
        "group_frequency_ns", "table_build_ns", "packed_writer_ns", "wrap_compare_ns",
    )
    result = {}
    for profile in ("compact", "low-latency"):
        selected = [row for row in rows if row["profile"] == profile]
        if len(selected) != 102 or any(row["production_exact"] != "1" for row in selected):
            raise RuntimeError(f"Phase A {profile} is incomplete or not byte exact")
        result[profile] = {
            "images": len(selected),
            "pixels": sum(int(row["pixels"]) for row in selected),
            "tokens": sum(int(row["tokens"]) for row in selected),
            "literals": sum(int(row["literals"]) for row in selected),
            "copies": sum(int(row["copies"]) for row in selected),
            "caches": sum(int(row["caches"]) for row in selected),
            "blocks": sum(int(row["blocks"]) for row in selected),
            "candidate_wins": sum(int(row["candidate_won"]) for row in selected),
            "phase_seconds": {
                phase.removesuffix("_ns"): sum(int(row[phase]) for row in selected) / 1e9
                for phase in phases
            },
        }
        seconds = result[profile]["phase_seconds"]
        result[profile]["theoretical_removable_seconds"] = (
            seconds["residual"] + seconds["block_census"] + seconds["group_frequency"]
        )
    return result


def main() -> None:
    root = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(__file__).parent
    variants = {}
    for label, directory in (
        ("initial-double-evaluation", "screen-41-variants-f5e5bee5"),
        ("fixed-streaming", "screen-41-variants-815df546"),
    ):
        stage = root / "raw" / directory
        variants[label] = {}
        for profile in ("compact", "low-latency"):
            variants[label][profile] = {
                name: pair(stage, f"{profile}-pipeline-control", f"{profile}-{layout}")
                for name, layout in (
                    ("S", "streaming"),
                    ("S+C", "streaming-census"),
                    ("S+C+F", "streaming-census-frequencies"),
                )
            }
    diagnostic_root = root / "raw" / "screen-41-materialized-cf-292c1d74"
    diagnostic = {
        profile: pair(
            diagnostic_root,
            f"{profile}-pipeline-control",
            f"{profile}-materialized-census-frequencies",
        )
        for profile in ("compact", "low-latency")
    }
    summary = {
        "decision": "reject",
        "formal_102x5": "skipped because every 41x3 screen failed the predeclared 10% and zero-regression gate",
        "phase_a": phase_a(root),
        "variant_screens": variants,
        "materialized_cf_diagnostic": diagnostic,
    }
    (root / "gate-summary.json").write_text(json.dumps(summary, indent=2, sort_keys=True) + "\n")


if __name__ == "__main__":
    main()
