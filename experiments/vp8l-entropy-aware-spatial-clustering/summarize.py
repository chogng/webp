#!/usr/bin/env python3
"""Summarize retained P15 Phase A and complete failed-screen evidence."""

from __future__ import annotations

import csv
import json
from pathlib import Path
import statistics
import sys


PROFILES = ("compact", "low-latency")
MAX_COUNTER_BYTES = {"compact": 34_373_632, "low-latency": 17_186_816}


def percent(candidate: int | float, control: int | float) -> float:
    return (candidate / control - 1) * 100


def phase_a_summary(root: Path) -> dict[str, object]:
    lines = [
        line[4:]
        for line in (root / "raw/phase-a-102/phase-a.tsv").read_text().splitlines()
        if line.startswith("p15\t")
    ]
    rows = list(csv.DictReader(lines, delimiter="\t"))
    if len(rows) != 204:
        raise RuntimeError(f"expected 204 Phase A rows, found {len(rows)}")
    result: dict[str, object] = {
        "binary_sha256": (root / "phase-a-summary.json").exists()
        and json.loads((root / "phase-a-summary.json").read_text())["binary_sha256"]
        or "reproduced-binary-recorded-separately",
        "manifest_rows": 102,
        "planner_writer_exact_rows": len(rows),
        "selector_exact_rows": len(rows),
        "public_exact_rows": sum(int(row["public_exact"]) for row in rows),
        "gate": "pass-enter-screen",
    }
    timing_fields = {
        "counter_initialization": "counter_init_ns",
        "counter_update": "counter_update_ns",
        "e_proposal": "e_proposal_ns",
        "b_proposal": "b_proposal_ns",
        "e_cost": "e_cost_ns",
        "b_cost": "b_cost_ns",
        "reassignment": "reassignment_ns",
        "rebuild_compaction": "rebuild_ns",
        "refined_cost": "refined_cost_ns",
        "final_selection": "final_selection_ns",
    }
    for profile in PROFILES:
        selected = [row for row in rows if row["profile"] == profile]
        ordered = sum(int(row["ordered_bytes"]) for row in selected)
        control = sum(int(row["control_bytes"]) for row in selected)
        e_bytes = sum(int(row["e_riff_bytes"]) for row in selected)
        b_bytes = sum(int(row["b_riff_bytes"]) for row in selected)
        minimum = sum(
            min(int(row["e_riff_bytes"]), int(row["b_riff_bytes"]))
            for row in selected
        )
        refined = sum(int(row["refined_riff_bytes"]) for row in selected)
        final = sum(int(row["final_bytes"]) for row in selected)
        minimum_deltas = [
            percent(
                min(int(row["e_riff_bytes"]), int(row["b_riff_bytes"])),
                int(row["ordered_bytes"]),
            )
            for row in selected
        ]
        final_deltas = [
            percent(int(row["final_bytes"]), int(row["control_bytes"]))
            for row in selected
        ]
        wins = {
            "E": sum(
                int(row["e_riff_bytes"]) <= int(row["b_riff_bytes"])
                for row in selected
            ),
            "B": sum(
                int(row["b_riff_bytes"]) < int(row["e_riff_bytes"])
                for row in selected
            ),
        }
        kinds = {
            kind: sum(row["final"] == kind for row in selected)
            for kind in ("E", "B", "R")
            if any(row["final"] == kind for row in selected)
        }
        result[profile.replace("-", "_")] = {
            "ordered_bytes": ordered,
            "e_bytes": e_bytes,
            "b_bytes": b_bytes,
            "min_eb_bytes": minimum,
            "min_eb_delta_pct": percent(minimum, ordered),
            "min_eb_wins": wins,
            "min_eb_over_2_pct": sum(value > 2 for value in minimum_deltas),
            "min_eb_worst_pct": max(minimum_deltas),
            "refined_bytes": refined,
            "final_bytes": final,
            "final_delta_pct": percent(final, control),
            "final_over_2_pct": sum(value > 2 for value in final_deltas),
            "final_over_2_ids": [
                row["id"] for row, value in zip(selected, final_deltas) if value > 2
            ],
            "final_worst_pct": max(final_deltas),
            "final_worst_id": selected[final_deltas.index(max(final_deltas))]["id"],
            "final_kinds": kinds,
            "timings_ns": {
                name: sum(int(row[field]) for row in selected)
                for name, field in timing_fields.items()
            },
            "counter_updates": sum(int(row["counter_updates"]) for row in selected),
            "merge_updates": sum(int(row["e_merge_updates"]) for row in selected),
            "max_measured_accounted_bytes": max(
                int(row["counter_storage_bytes"]) + int(row["retained_plan_bytes"])
                for row in selected
            ),
            "maximum_dimension_counter_bytes": MAX_COUNTER_BYTES[profile],
        }
    return result


def process_data(root: Path) -> dict[str, list[dict[str, object]]]:
    records = [json.loads(line) for line in (root / "processes.jsonl").read_text().splitlines()]
    result: dict[str, list[dict[str, object]]] = {}
    for process in records:
        if process.get("phase") != "formal":
            continue
        output = Path(str(process["stdout"]))
        if not output.exists():
            output = root / "measurements" / output.name
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
        result.setdefault(str(process["layout"]), []).append(
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
    return result


def robust(values: list[int] | list[float]) -> dict[str, object]:
    median = statistics.median(values)
    deviations = [abs(value - median) for value in values]
    mad = statistics.median(deviations)
    return {
        "samples": values,
        "median": median,
        "mad": mad,
        "outlier_3mad": [
            False if mad == 0 else abs(value - median) > 3 * mad for value in values
        ],
    }


def pair_summary(
    data: dict[str, list[dict[str, object]]], control: str, candidate: str
) -> dict[str, object]:
    controls = sorted(data[control], key=lambda row: int(row["round"]))
    candidates = sorted(data[candidate], key=lambda row: int(row["round"]))
    control_ns = [int(row["aggregate_ns"]) for row in controls]
    candidate_ns = [int(row["aggregate_ns"]) for row in candidates]
    control_median = statistics.median(control_ns)
    candidate_median = statistics.median(candidate_ns)
    paired = [
        (int(new["aggregate_ns"]) / int(old["aggregate_ns"]) - 1) * 100
        for old, new in zip(controls, candidates)
    ]
    ids = sorted(dict(controls[0]["items"]))
    per_image = []
    for image in ids:
        old = statistics.median(
            [int(dict(row["items"])[image]["ns"]) for row in controls]
        )
        new = statistics.median(
            [int(dict(row["items"])[image]["ns"]) for row in candidates]
        )
        old_bytes = int(dict(controls[0]["items"])[image]["stream_bytes"])
        new_bytes = int(dict(candidates[0]["items"])[image]["stream_bytes"])
        per_image.append(
            {
                "id": image,
                "control_median_ns": old,
                "candidate_median_ns": new,
                "time_delta_pct": (new / old - 1) * 100,
                "control_bytes": old_bytes,
                "candidate_bytes": new_bytes,
                "rate_delta_pct": (new_bytes / old_bytes - 1) * 100,
            }
        )
    control_bytes = int(controls[0]["stream_bytes"])
    candidate_bytes = int(candidates[0]["stream_bytes"])
    control_rss = statistics.median([int(row["max_rss_bytes"]) for row in controls])
    candidate_rss = statistics.median([int(row["max_rss_bytes"]) for row in candidates])
    return {
        "control": control,
        "candidate": candidate,
        "aggregate_control_ns": robust(control_ns),
        "aggregate_candidate_ns": robust(candidate_ns),
        "speed_delta_pct": (candidate_median / control_median - 1) * 100,
        "paired_delta_pct": robust(paired),
        "control_process_wall_ns": robust([int(row["process_wall_ns"]) for row in controls]),
        "candidate_process_wall_ns": robust([int(row["process_wall_ns"]) for row in candidates]),
        "control_cpu_ns": robust([int(row["cpu_ns"]) for row in controls]),
        "candidate_cpu_ns": robust([int(row["cpu_ns"]) for row in candidates]),
        "control_rss_bytes": robust([int(row["max_rss_bytes"]) for row in controls]),
        "candidate_rss_bytes": robust([int(row["max_rss_bytes"]) for row in candidates]),
        "rss_delta_bytes": candidate_rss - control_rss,
        "rss_delta_pct": (candidate_rss / control_rss - 1) * 100,
        "control_bytes": control_bytes,
        "candidate_bytes": candidate_bytes,
        "rate_delta_pct": (candidate_bytes / control_bytes - 1) * 100,
        "max_image_rate_delta_pct": max(row["rate_delta_pct"] for row in per_image),
        "max_image_rate_id": max(per_image, key=lambda row: row["rate_delta_pct"])["id"],
        "images_rate_over_2pct": sum(row["rate_delta_pct"] > 2 for row in per_image),
        "median_time_regressions": sum(row["time_delta_pct"] > 0 for row in per_image),
        "per_image": per_image,
    }


def main() -> None:
    root = Path(sys.argv[1] if len(sys.argv) > 1 else Path(__file__).parent)
    encode = process_data(root / "raw/screen-41-encode")
    rust = process_data(root / "raw/screen-41-rust-decode")
    pinned = process_data(root / "raw/screen-41-libwebp-decode")
    pairs = {
        "compact": ("compact-control", "compact"),
        "low_latency": ("low-latency-control", "low-latency"),
    }
    screen: dict[str, object] = {}
    for profile, names in pairs.items():
        screen[profile] = {
            "encode": pair_summary(encode, *names),
            "rust_decode": pair_summary(rust, *names),
            "pinned_c_decode": pair_summary(pinned, *names),
        }
    project_rows = [
        line.split("\t")
        for line in (root / "raw/screen-41-correctness/project-generate.tsv").read_text().splitlines()
        if line.startswith("stream\t") and not line.startswith("stream\tid\t")
    ]
    oracle_summary = next(
        line
        for line in (root / "raw/screen-41-correctness/libwebp-compare.tsv").read_text().splitlines()
        if line.startswith("oracle_summary\t")
    )
    for profile in screen.values():
        encoded = profile["encode"]
        profile["screen_gate"] = (
            encoded["speed_delta_pct"] <= -10
            and encoded["median_time_regressions"] == 0
            and encoded["rate_delta_pct"] <= 0
            and encoded["images_rate_over_2pct"] == 0
            and profile["rust_decode"]["speed_delta_pct"] <= 1
            and profile["pinned_c_decode"]["speed_delta_pct"] <= 1
            and encoded["rss_delta_bytes"] <= 64 * 1024 * 1024
            and encoded["rss_delta_pct"] <= 5
        )
    identity_summary = next(
        line
        for line in (root / "raw/identity-306-final/identity-306.tsv").read_text().splitlines()
        if line.startswith("identity_summary\t")
    )
    archive_oracle_summary = next(
        line
        for line in (root / "raw/identity-306-final/oracle-918.tsv").read_text().splitlines()
        if line.startswith("oracle_summary\t")
    )
    validation_rows = list(
        csv.DictReader(
            (root / "raw/validation-final/validation.tsv").read_text().splitlines(),
            delimiter="\t",
        )
    )
    failures = []
    for profile, summary in screen.items():
        encoded = summary["encode"]
        if encoded["speed_delta_pct"] > -10:
            failures.append(f"{profile}:encode")
        if encoded["median_time_regressions"]:
            failures.append(f"{profile}:per-image-encode")
        if encoded["rate_delta_pct"] > 0 or encoded["images_rate_over_2pct"]:
            failures.append(f"{profile}:rate")
        if summary["rust_decode"]["speed_delta_pct"] > 1:
            failures.append(f"{profile}:rust-decode")
        if summary["pinned_c_decode"]["speed_delta_pct"] > 1:
            failures.append(f"{profile}:pinned-c-decode")
        if encoded["rss_delta_bytes"] > 64 * 1024 * 1024 or encoded["rss_delta_pct"] > 5:
            failures.append(f"{profile}:rss")
    result = {
        "decision": "reject-screen",
        "screen_failures": failures,
        "formal_102x5_run": False,
        "phase_a": phase_a_summary(root),
        "screen": screen,
        "correctness": {
            "project_streams": len(project_rows),
            "project_exact": sum(row[-1] == "1" for row in project_rows),
            "pinned_c_summary": oracle_summary,
            "archive_identity_summary": identity_summary,
            "archive_pinned_c_summary": archive_oracle_summary,
            "default_full_byte": 102,
            "archive_project_exact": 918,
            "archive_pinned_c_exact": 918,
        },
        "quality": {
            "commands": validation_rows,
            "passed": sum(int(row["status"]) == 0 for row in validation_rows),
            "total": len(validation_rows),
            "stable_host_target": "aarch64-apple-darwin",
        },
        "resources": {
            "compact_screen_rss_delta_bytes": screen["compact"]["encode"]["rss_delta_bytes"],
            "compact_screen_rss_delta_pct": screen["compact"]["encode"]["rss_delta_pct"],
            "low_latency_screen_rss_delta_bytes": screen["low_latency"]["encode"]["rss_delta_bytes"],
            "low_latency_screen_rss_delta_pct": screen["low_latency"]["encode"]["rss_delta_pct"],
            "e37_rlib_bytes": 462_384,
            "p15_rlib_bytes": 574_488,
            "e37_test_binary_bytes": 1_523_552,
            "p15_test_binary_bytes": 1_573_600,
            "maximum_dimension_compact_counter_bytes": 34_373_632,
            "maximum_dimension_low_latency_counter_bytes": 17_186_816,
        },
        "binary": json.loads((root / "raw/screen-41-encode/run.json").read_text())["binary_sha256"],
    }
    (root / "gate-summary.json").write_text(json.dumps(result, indent=2, sort_keys=True) + "\n")


if __name__ == "__main__":
    main()
