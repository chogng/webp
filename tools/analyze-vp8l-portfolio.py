#!/usr/bin/env python3
"""Calibrate the bounded VP8L portfolio from disjoint decoder measurements."""

from __future__ import annotations

import argparse
import csv
import statistics
from collections import defaultdict
from pathlib import Path


LAYOUTS = ("single", "compact-candidate", "low-latency-candidate")
WEIGHTS = (1, 2, 4, 8, 16, 32, 64, 128, 256, 512, 1024)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--audit", type=Path, required=True)
    parser.add_argument("--project", type=Path, required=True)
    parser.add_argument("--libwebp", type=Path, required=True)
    return parser.parse_args()


def audit_rows(path: Path) -> dict[str, list[dict[str, int]]]:
    lines = [line for line in path.read_text().splitlines() if line.startswith("portfolio\t")]
    reader = csv.DictReader(lines, delimiter="\t")
    rows: dict[str, list[dict[str, int]]] = {}
    for row in reader:
        if row["profile"] != "compact":
            continue
        rows[row["id"]] = [
            {
                "bits": int(row[f"{name}_bits"]),
                "secondary": int(row[f"{name}_secondary"]),
                "runs": int(row[f"{name}_runs"]),
                "memory": int(row[f"{name}_memory"]),
            }
            for name in ("single", "compact", "low_latency")
        ]
    if not rows:
        raise SystemExit("audit contains no compact portfolio rows")
    return rows


def project_times(root: Path) -> dict[tuple[str, str], float]:
    values: dict[tuple[str, str], list[float]] = defaultdict(list)
    for path in root.glob("*-r*-decode-*.tsv"):
        for line in path.read_text().splitlines():
            fields = line.split("\t")
            if fields[:2] != ["measurement", "decode"]:
                continue
            values[(fields[4], fields[3])].append(float(fields[5]))
    return medians(values, "project")


def libwebp_times(root: Path) -> dict[tuple[str, str], float]:
    values: dict[tuple[str, str], list[float]] = defaultdict(list)
    for path in root.glob("r*-*.tsv"):
        layout = path.stem.split("-", 1)[1]
        for line in path.read_text().splitlines():
            fields = line.split("\t")
            if fields[:2] != ["measurement", "libwebp"]:
                continue
            identifier = Path(fields[2]).stem
            iterations = int(fields[3])
            nanoseconds = float(fields[4]) * 1_000_000.0 / iterations
            values[(identifier, layout)].append(nanoseconds)
    return medians(values, "libwebp")


def medians(
    values: dict[tuple[str, str], list[float]], decoder: str
) -> dict[tuple[str, str], float]:
    result = {key: statistics.median(samples) for key, samples in values.items()}
    if not result:
        raise SystemExit(f"{decoder} measurements are empty")
    return result


def select(costs: list[dict[str, int]], latency: int, memory: int) -> int:
    candidate_indices = (0, 2)
    floor = min(costs[index]["bits"] for index in candidate_indices)
    ceiling = floor + floor // 100
    eligible = [
        (index, costs[index])
        for index in candidate_indices
        if costs[index]["bits"] <= ceiling
    ]
    return min(
        eligible,
        key=lambda item: (
            item[1]["bits"]
            + latency * (item[1]["secondary"] + 8 * item[1]["runs"])
            + memory * (item[1]["memory"] // 64),
            item[1]["bits"],
            item[0],
        ),
    )[0]


def evaluate(
    audit: dict[str, list[dict[str, int]]],
    times: dict[tuple[str, str], float],
    latency: int,
    memory: int,
) -> tuple[float, float, tuple[int, int, int], int]:
    selected_ns = 0.0
    oracle_ns = 0.0
    counts = [0, 0, 0]
    selected_bits = 0
    for identifier, costs in audit.items():
        available = [times[(identifier, layout)] for layout in LAYOUTS]
        candidate_indices = (0, 2)
        floor = min(costs[index]["bits"] for index in candidate_indices)
        ceiling = floor + floor // 100
        eligible = [
            index
            for index in candidate_indices
            if costs[index]["bits"] <= ceiling
        ]
        winner = select(costs, latency, memory)
        counts[winner] += 1
        selected_bits += costs[winner]["bits"]
        selected_ns += available[winner]
        oracle_ns += min(available[index] for index in eligible)
    return selected_ns, oracle_ns, tuple(counts), selected_bits


def main() -> int:
    args = parse_args()
    audit = audit_rows(args.audit)
    decoders = {
        "project": project_times(args.project),
        "libwebp": libwebp_times(args.libwebp),
    }
    for identifier in audit:
        for layout in LAYOUTS:
            for name, times in decoders.items():
                if (identifier, layout) not in times:
                    raise SystemExit(f"missing {name} measurement for {identifier}/{layout}")

    candidates = []
    for latency in WEIGHTS:
        for memory in WEIGHTS:
            results = {
                name: evaluate(audit, times, latency, memory)
                for name, times in decoders.items()
            }
            worst_regret = max(
                selected / oracle for selected, oracle, _, _ in results.values()
            )
            mean_regret = sum(
                selected / oracle for selected, oracle, _, _ in results.values()
            ) / len(results)
            candidates.append((worst_regret, mean_regret, latency, memory, results))
    _, _, latency, memory, results = min(candidates)

    print(f"images\t{len(audit)}")
    print(f"latency_weight\t{latency}")
    print(f"memory_weight\t{memory}")
    for name, (selected, oracle, counts, bits) in results.items():
        print(
            f"decoder\t{name}\tselected_ns\t{selected:.0f}\toracle_ns\t{oracle:.0f}"
            f"\tregret\t{selected / oracle:.9f}\tcounts\t{counts}\tbits\t{bits}"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
