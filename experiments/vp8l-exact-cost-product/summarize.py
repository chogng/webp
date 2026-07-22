#!/usr/bin/env python3
"""Summarize locked exact-cost screen and formal product gates."""

from __future__ import annotations

import json
from pathlib import Path
import statistics
import sys


def robust(values: list[float]) -> dict[str, object]:
    median = statistics.median(values)
    deviations = [abs(value - median) for value in values]
    mad = statistics.median(deviations)
    return {
        "values": values,
        "median": median,
        "mad": mad,
        "three_mad_outlier_rounds": [
            index + 1
            for index, value in enumerate(values)
            if mad != 0 and abs(value - median) > 3 * mad
        ],
    }


def load_aggregates(root: Path) -> dict[str, dict[int, dict[str, object]]]:
    aggregates: dict[str, dict[int, dict[str, object]]] = {}
    for path in sorted((root / "measurements").glob("*-r*-encode-*.tsv")):
        for line in path.read_text().splitlines():
            fields = line.split("\t")
            if fields[0] == "aggregate" and fields[1] == "encode":
                aggregates.setdefault(fields[3], {})[int(fields[2])] = {
                    "wall_ns": int(fields[5]),
                    "input_bytes": int(fields[6]),
                    "output_bytes": int(fields[7]),
                    "checksum": fields[8],
                }
    return aggregates


def load_images(root: Path) -> dict[str, dict[str, dict[int, int]]]:
    images: dict[str, dict[str, dict[int, int]]] = {}
    for path in sorted((root / "measurements").glob("*-r*-encode-*.tsv")):
        for line in path.read_text().splitlines():
            fields = line.split("\t")
            if fields[0] == "measurement" and fields[1] == "encode":
                images.setdefault(fields[3], {}).setdefault(fields[4], {})[
                    int(fields[2])
                ] = int(fields[5])
    return images


def percentile(values: list[float], fraction: float) -> float:
    ordered = sorted(values)
    position = fraction * (len(ordered) - 1)
    lower = int(position)
    upper = min(lower + 1, len(ordered) - 1)
    weight = position - lower
    return ordered[lower] * (1 - weight) + ordered[upper] * weight


def load_processes(root: Path) -> list[dict[str, object]]:
    return [
        json.loads(line)
        for line in (root / "processes.jsonl").read_text().splitlines()
        if json.loads(line).get("phase") == "formal"
    ]


def pair(root: Path, baseline: str, candidate: str) -> dict[str, object]:
    aggregates = load_aggregates(root)
    rounds = sorted(aggregates[baseline])
    if rounds != sorted(aggregates[candidate]):
        raise RuntimeError(f"{root}: unmatched rounds")
    control = [int(aggregates[baseline][number]["wall_ns"]) for number in rounds]
    exact = [int(aggregates[candidate][number]["wall_ns"]) for number in rounds]
    ratios = [100 * (value / base - 1) for base, value in zip(control, exact)]
    outputs = {
        layout: {
            (
                int(record["input_bytes"]),
                int(record["output_bytes"]),
                str(record["checksum"]),
            )
            for record in layout_rounds.values()
        }
        for layout, layout_rounds in aggregates.items()
        if layout in (baseline, candidate)
    }
    if any(len(values) != 1 for values in outputs.values()):
        raise RuntimeError(f"{root}: unstable output")
    if outputs[baseline] != outputs[candidate]:
        raise RuntimeError(f"{root}: control/candidate output mismatch")
    input_bytes, output_bytes, checksum = next(iter(outputs[baseline]))
    result: dict[str, object] = {
        "control_wall_ns": robust(control),
        "exact_wall_ns": robust(exact),
        "independent_ratio_percent": 100
        * (statistics.median(exact) / statistics.median(control) - 1),
        "paired_ratio_percent": robust(ratios),
        "stream": {
            "input_bytes": input_bytes,
            "output_bytes": output_bytes,
            "checksum": checksum,
            "control_exact": True,
        },
    }
    images = load_images(root)
    per_image = [
        100
        * (
            statistics.median(images[candidate][image].values())
            / statistics.median(control_rounds.values())
            - 1
        )
        for image, control_rounds in images[baseline].items()
    ]
    result["per_image_ratio_percentiles"] = {
        name: percentile(per_image, fraction)
        for name, fraction in (("p0", 0), ("p10", 0.1), ("p50", 0.5), ("p90", 0.9), ("p100", 1))
    }
    processes = load_processes(root)
    for name, layout in (("control", baseline), ("exact", candidate)):
        selected = sorted(
            (record for record in processes if record["layout"] == layout),
            key=lambda record: record["round"],
        )
        result[f"{name}_process_wall_ns"] = robust(
            [int(record["process_wall_ns"]) for record in selected]
        )
        result[f"{name}_cpu_ns"] = robust(
            [int(record["user_ns"]) + int(record["sys_ns"]) for record in selected]
        )
        result[f"{name}_max_rss_bytes"] = robust(
            [int(record["max_rss_bytes"]) for record in selected]
        )
    return result


def main() -> None:
    root = Path(sys.argv[1] if len(sys.argv) > 1 else ".")
    summary = {
        "screen": {
            "compact": pair(root / "screen-compact", "compact-control", "compact"),
            "low_latency": pair(
                root / "screen-low-latency", "low-latency-control", "low-latency"
            ),
        },
        "formal": {
            "compact": pair(root / "formal-102", "compact-control", "compact"),
            "low_latency": pair(
                root / "formal-102", "low-latency-control", "low-latency"
            ),
        },
    }
    summary["gate"] = {
        profile: {
            "at_least_25_percent": summary["formal"][profile][
                "independent_ratio_percent"
            ]
            <= -25,
            "at_most_11_seconds": summary["formal"][profile]["exact_wall_ns"][
                "median"
            ]
            <= 11_000_000_000,
        }
        for profile in ("compact", "low_latency")
    }
    (root / "gate-summary.json").write_text(
        json.dumps(summary, indent=2, sort_keys=True) + "\n"
    )
    lines = [
        "# Exact-cost product gate summary",
        "",
        "| gate | profile | control s | product s | change | paired |",
        "| --- | --- | ---: | ---: | ---: | ---: |",
    ]
    for gate in ("screen", "formal"):
        for profile in ("compact", "low_latency"):
            item = summary[gate][profile]
            lines.append(
                f"| {gate} | {profile} | {item['control_wall_ns']['median'] / 1e9:.6f} | "
                f"{item['exact_wall_ns']['median'] / 1e9:.6f} | "
                f"{item['independent_ratio_percent']:+.3f}% | "
                f"{item['paired_ratio_percent']['median']:+.3f}% |"
            )
    lines.append("")
    (root / "gate-summary.md").write_text("\n".join(lines))
    if not all(
        value
        for profile in summary["gate"].values()
        for value in profile.values()
    ):
        raise SystemExit(f"formal product gate failed: {summary['gate']}")


if __name__ == "__main__":
    main()
