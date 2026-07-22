#!/usr/bin/env python3
"""Summarize locked VP8L product measurements with robust statistics."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import statistics


def percentile(values: list[float], fraction: float) -> float:
    ordered = sorted(values)
    position = fraction * (len(ordered) - 1)
    lower = int(position)
    upper = min(lower + 1, len(ordered) - 1)
    weight = position - lower
    return ordered[lower] * (1 - weight) + ordered[upper] * weight


def robust(values: list[float]) -> dict[str, object]:
    median = statistics.median(values)
    deviations = [abs(value - median) for value in values]
    mad = statistics.median(deviations)
    outliers = [
        index + 1
        for index, value in enumerate(values)
        if mad != 0 and abs(value - median) > 3 * mad
    ]
    retained = [value for index, value in enumerate(values, 1) if index not in outliers]
    return {
        "values": values,
        "median": median,
        "mad": mad,
        "three_mad_outlier_rounds": outliers,
        "median_without_outliers": statistics.median(retained),
    }


def load_measurements(root: Path) -> tuple[dict, dict]:
    aggregates: dict[str, dict[str, dict[int, int]]] = {}
    images: dict[str, dict[str, dict[str, dict[int, int]]]] = {}
    for path in sorted((root / "measurements").glob("*-r*-*.tsv")):
        for line in path.read_text().splitlines():
            fields = line.split("\t")
            if fields[0] == "aggregate":
                _, operation, round_text, layout, _, elapsed, *_ = fields
                aggregates.setdefault(operation, {}).setdefault(layout, {})[
                    int(round_text)
                ] = int(elapsed)
            elif fields[0] == "measurement":
                _, operation, round_text, layout, image, elapsed, *_ = fields
                images.setdefault(operation, {}).setdefault(layout, {}).setdefault(image, {})[
                    int(round_text)
                ] = int(elapsed)
    return aggregates, images


def summarize(root: Path) -> dict[str, object]:
    run = json.loads((root / "run.json").read_text())
    layouts = run["layouts"]
    aggregates, images = load_measurements(root)
    processes = [
        json.loads(line) for line in (root / "processes.jsonl").read_text().splitlines()
    ]
    result: dict[str, object] = {"run": run, "operations": {}}
    baseline_layout = run.get("baseline", "single")
    for operation in run["operations"]:
        operation_result: dict[str, object] = {"layouts": {}, "comparisons": {}}
        rounds = sorted(aggregates[operation][baseline_layout])
        baseline = [aggregates[operation][baseline_layout][number] for number in rounds]
        for layout in layouts:
            values = [aggregates[operation][layout][number] for number in rounds]
            selected = [
                record
                for record in processes
                if record.get("phase") == "formal"
                and record["operation"] == operation
                and record["layout"] == layout
            ]
            selected.sort(key=lambda record: record["round"])
            operation_result["layouts"][layout] = {
                "aggregate_wall_ns": robust(values),
                "process_wall_ns": robust(
                    [record["process_wall_ns"] for record in selected]
                ),
                "cpu_ns": robust(
                    [record["user_ns"] + record["sys_ns"] for record in selected]
                ),
                "max_rss_bytes": robust(
                    [record["max_rss_bytes"] for record in selected]
                ),
            }
            if layout == baseline_layout:
                continue
            ratios = [
                100
                * (
                    aggregates[operation][layout][number]
                    / aggregates[operation][baseline_layout][number]
                    - 1
                )
                for number in rounds
            ]
            per_image = []
            worst = None
            for image, baseline_rounds in images[operation][baseline_layout].items():
                candidate_rounds = images[operation][layout][image]
                ratio = 100 * (
                    statistics.median(candidate_rounds.values())
                    / statistics.median(baseline_rounds.values())
                    - 1
                )
                per_image.append(ratio)
                if worst is None or ratio > worst[1]:
                    worst = (image, ratio)
            candidate = [aggregates[operation][layout][number] for number in rounds]
            operation_result["comparisons"][layout] = {
                "independent_median_ratio_percent": 100
                * (statistics.median(candidate) / statistics.median(baseline) - 1),
                "paired_ratio_percent": robust(ratios),
                "per_image_ratio_percentiles": {
                    name: percentile(per_image, fraction)
                    for name, fraction in (
                        ("p0", 0),
                        ("p10", 0.1),
                        ("p50", 0.5),
                        ("p90", 0.9),
                        ("p100", 1),
                    )
                },
                "worst_image": {"image": worst[0], "ratio_percent": worst[1]},
            }
        result["operations"][operation] = operation_result
    return result


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("root", type=Path)
    args = parser.parse_args()
    summary = summarize(args.root)
    (args.root / "summary.json").write_text(
        json.dumps(summary, indent=2, sort_keys=True) + "\n"
    )
    lines = [
        "# Locked VP8L product benchmark summary",
        "",
        "| operation | layout | median s | MAD s | vs baseline | paired median | wall outlier rounds |",
        "| --- | --- | ---: | ---: | ---: | ---: | --- |",
    ]
    for operation, values in summary["operations"].items():
        baseline_layout = summary["run"].get("baseline", "single")
        baseline = values["layouts"][baseline_layout]["aggregate_wall_ns"]
        lines.append(
            f"| {operation} | {baseline_layout} | {baseline['median'] / 1e9:.6f} | {baseline['mad'] / 1e9:.6f} | — | — | {baseline['three_mad_outlier_rounds']} |"
        )
        for layout, comparison in values["comparisons"].items():
            wall = values["layouts"][layout]["aggregate_wall_ns"]
            paired = comparison["paired_ratio_percent"]
            lines.append(
                f"| {operation} | {layout} | {wall['median'] / 1e9:.6f} | {wall['mad'] / 1e9:.6f} | {comparison['independent_median_ratio_percent']:+.3f}% | {paired['median']:+.3f}% | {wall['three_mad_outlier_rounds']} |"
            )
    lines.extend(
        [
            "",
            "Full CPU, RSS, per-image, paired-round, MAD, and 3×MAD data are in `summary.json`.",
            "",
        ]
    )
    (args.root / "summary.md").write_text("\n".join(lines))


if __name__ == "__main__":
    main()
