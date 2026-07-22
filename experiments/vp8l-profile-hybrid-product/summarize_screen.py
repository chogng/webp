#!/usr/bin/env python3
"""Summarize the locked P20 41-image screen without deleting samples."""

from __future__ import annotations

import argparse
import csv
import json
from pathlib import Path
import statistics


PAIRS = {
    "compact": ("compact-control", "compact"),
    "low-latency": ("low-latency-control", "low-latency"),
}


def robust(values: list[int | float]) -> dict[str, object]:
    median = statistics.median(values)
    mad = statistics.median(abs(value - median) for value in values)
    return {
        "samples": values,
        "median": median,
        "mad": mad,
        "three_mad_flags": [
            False if mad == 0 else abs(value - median) > 3 * mad for value in values
        ],
    }


def measured(root: Path) -> dict[str, list[dict[str, object]]]:
    result: dict[str, list[dict[str, object]]] = {}
    for line in (root / "processes.jsonl").read_text().splitlines():
        process = json.loads(line)
        if process.get("phase") != "formal":
            continue
        output = Path(process["stdout"])
        rows = list(csv.reader(output.read_text().splitlines(), delimiter="\t"))
        aggregate = next(row for row in rows if row and row[0] == "aggregate")
        items = {
            row[4]: {
                "ns": int(row[5]),
                "rgba_bytes": int(row[6]),
                "stream_bytes": int(row[7]),
                "hash": row[8],
            }
            for row in rows
            if row and row[0] == "measurement"
        }
        result.setdefault(process["layout"], []).append(
            {
                "round": int(process["round"]),
                "aggregate_ns": int(aggregate[5]),
                "rgba_bytes": int(aggregate[6]),
                "stream_bytes": int(aggregate[7]),
                "process_wall_ns": int(process["process_wall_ns"]),
                "cpu_ns": int(process["user_ns"]) + int(process["sys_ns"]),
                "max_rss_bytes": int(process["max_rss_bytes"]),
                "items": items,
            }
        )
    for records in result.values():
        records.sort(key=lambda record: int(record["round"]))
    return result


def pair_summary(
    samples: dict[str, list[dict[str, object]]], control: str, candidate: str
) -> dict[str, object]:
    controls = samples[control]
    candidates = samples[candidate]
    if len(controls) != 3 or len(candidates) != 3:
        raise ValueError("screen requires exactly three measured rounds per layout")
    control_ns = [int(row["aggregate_ns"]) for row in controls]
    candidate_ns = [int(row["aggregate_ns"]) for row in candidates]
    control_median = statistics.median(control_ns)
    candidate_median = statistics.median(candidate_ns)
    image_ratios: list[tuple[str, float]] = []
    byte_ratios: list[tuple[str, float]] = []
    for image in sorted(controls[0]["items"]):
        old = statistics.median(int(row["items"][image]["ns"]) for row in controls)
        new = statistics.median(int(row["items"][image]["ns"]) for row in candidates)
        image_ratios.append((image, 100 * (new / old - 1)))
        old_bytes = int(controls[0]["items"][image]["stream_bytes"])
        new_bytes = int(candidates[0]["items"][image]["stream_bytes"])
        byte_ratios.append((image, 100 * (new_bytes / old_bytes - 1)))
    control_bytes = int(controls[0]["stream_bytes"])
    candidate_bytes = int(candidates[0]["stream_bytes"])
    control_rss = [int(row["max_rss_bytes"]) for row in controls]
    candidate_rss = [int(row["max_rss_bytes"]) for row in candidates]
    rss_delta = statistics.median(candidate_rss) - statistics.median(control_rss)
    return {
        "control_aggregate_ns": robust(control_ns),
        "candidate_aggregate_ns": robust(candidate_ns),
        "independent_delta_pct": 100 * (candidate_median / control_median - 1),
        "paired_delta_pct": robust(
            [100 * (new / old - 1) for old, new in zip(control_ns, candidate_ns)]
        ),
        "per_image_regressions": sum(ratio > 0 for _, ratio in image_ratios),
        "worst_image": max(image_ratios, key=lambda item: item[1]),
        "control_bytes": control_bytes,
        "candidate_bytes": candidate_bytes,
        "aggregate_bytes_delta_pct": 100 * (candidate_bytes / control_bytes - 1),
        "images_over_control_plus_2pct": sum(ratio > 2 for _, ratio in byte_ratios),
        "worst_image_bytes": max(byte_ratios, key=lambda item: item[1]),
        "control_rss_bytes": robust(control_rss),
        "candidate_rss_bytes": robust(candidate_rss),
        "rss_delta_bytes": rss_delta,
        "rss_delta_pct": 100 * (rss_delta / statistics.median(control_rss)),
    }


def count_project_exact(path: Path) -> tuple[int, int]:
    rows = [
        line.split("\t")
        for line in path.read_text().splitlines()
        if line.startswith("stream\t") and not line.startswith("stream\tid\t")
    ]
    return sum(row[-1] == "1" for row in rows), len(rows)


def pinned_exact(path: Path) -> tuple[int, int]:
    summary = next(
        line for line in path.read_text().splitlines() if line.startswith("oracle_summary\t")
    )
    fields = dict(field.split("=") for field in summary.split("\t")[1:])
    return int(fields["matched"]), int(fields["failed"])


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--encode", type=Path, required=True)
    parser.add_argument("--rust-decode", type=Path, required=True)
    parser.add_argument("--c-decode", type=Path, required=True)
    parser.add_argument("--project-generate", type=Path, required=True)
    parser.add_argument("--pinned-compare", type=Path, required=True)
    parser.add_argument("--binary-sha256", required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()

    encode_run = json.loads((args.encode / "run.json").read_text())
    rust_run = json.loads((args.rust_decode / "run.json").read_text())
    binary_exact = (
        encode_run["binary_sha256"]
        == rust_run["binary_sha256"]
        == args.binary_sha256
    )
    encode_samples = measured(args.encode)
    rust_samples = measured(args.rust_decode)
    c_samples = measured(args.c_decode)
    project_matched, project_total = count_project_exact(args.project_generate)
    pinned_matched, pinned_failed = pinned_exact(args.pinned_compare)
    profiles: dict[str, object] = {}
    gate = binary_exact and project_matched == project_total == 246
    gate = gate and pinned_matched == 246 and pinned_failed == 0
    for profile, (control, candidate) in PAIRS.items():
        encode = pair_summary(encode_samples, control, candidate)
        rust = pair_summary(rust_samples, control, candidate)
        pinned = pair_summary(c_samples, control, candidate)
        profile_gate = (
            float(encode["independent_delta_pct"]) <= -50
            and int(encode["per_image_regressions"]) == 0
            and int(encode["candidate_bytes"]) <= int(encode["control_bytes"])
            and int(encode["images_over_control_plus_2pct"]) == 0
            and float(rust["independent_delta_pct"]) < 1
            and float(pinned["independent_delta_pct"]) < 1
            and int(encode["rss_delta_bytes"]) < 64 * 1024 * 1024
            and float(encode["rss_delta_pct"]) < 5
        )
        profiles[profile] = {
            "gate": profile_gate,
            "encode": encode,
            "rust_decode": rust,
            "pinned_c_decode": pinned,
        }
        gate = gate and profile_gate
    stderr_files = sorted(args.output.parent.glob("**/*.stderr"))
    stderr_bytes = sum(path.stat().st_size for path in stderr_files)
    gate = gate and stderr_bytes == 0
    result = {
        "binary_sha256": args.binary_sha256,
        "gate": gate,
        "profiles": profiles,
        "correctness": {
            "project": f"{project_matched}/{project_total}",
            "pinned_c": f"{pinned_matched}/{pinned_matched + pinned_failed}",
        },
        "stderr_bytes": stderr_bytes,
    }
    args.output.write_text(json.dumps(result, indent=2, sort_keys=True) + "\n")
    if not gate:
        raise SystemExit("screen gate failed")


if __name__ == "__main__":
    main()
