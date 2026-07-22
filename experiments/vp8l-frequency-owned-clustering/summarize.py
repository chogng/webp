#!/usr/bin/env python3
"""Summarize retained P14 Phase A/B and exact-symbol screen evidence."""

from __future__ import annotations

import csv
import json
from pathlib import Path
import statistics
import sys


def tsv_rows(path: Path, prefix: str) -> list[dict[str, str]]:
    lines = [line for line in path.read_text().splitlines() if line.startswith(prefix + "\t")]
    return list(csv.DictReader(lines, delimiter="\t"))


def process_data(root: Path) -> dict[str, list[dict[str, object]]]:
    records = [json.loads(line) for line in (root / "processes.jsonl").read_text().splitlines()]
    result: dict[str, list[dict[str, object]]] = {}
    for process in records:
        if process.get("phase") != "formal":
            continue
        rows = list(csv.reader(Path(process["stdout"]).read_text().splitlines(), delimiter="\t"))
        aggregate = next(row for row in rows if row and row[0] == "aggregate")
        items = {
            row[4]: {"ns": int(row[5]), "rgba_bytes": int(row[6]), "stream_bytes": int(row[7]), "hash": row[8]}
            for row in rows
            if row and row[0] == "measurement"
        }
        result.setdefault(process["layout"], []).append(
            {
                "round": process["round"],
                "aggregate_ns": int(aggregate[5]),
                "rgba_bytes": int(aggregate[6]),
                "stream_bytes": int(aggregate[7]),
                "process_wall_ns": process["process_wall_ns"],
                "cpu_ns": process["user_ns"] + process["sys_ns"],
                "max_rss_bytes": process["max_rss_bytes"],
                "items": items,
            }
        )
    return result


def robust(values: list[int]) -> dict[str, object]:
    median = statistics.median(values)
    deviations = [abs(value - median) for value in values]
    mad = statistics.median(deviations)
    return {
        "samples": values,
        "median": median,
        "mad": mad,
        "outlier_3mad": [False if mad == 0 else abs(value - median) > 3 * mad for value in values],
    }


def pair_summary(data: dict[str, list[dict[str, object]]], control: str, candidate: str) -> dict[str, object]:
    controls = data[control]
    candidates = data[candidate]
    control_ns = [int(row["aggregate_ns"]) for row in controls]
    candidate_ns = [int(row["aggregate_ns"]) for row in candidates]
    control_median = statistics.median(control_ns)
    candidate_median = statistics.median(candidate_ns)
    ids = sorted(controls[0]["items"])
    per_image = []
    for image in ids:
        old = statistics.median([int(row["items"][image]["ns"]) for row in controls])
        new = statistics.median([int(row["items"][image]["ns"]) for row in candidates])
        old_bytes = int(controls[0]["items"][image]["stream_bytes"])
        new_bytes = int(candidates[0]["items"][image]["stream_bytes"])
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
        "images_rate_over_2pct": sum(row["rate_delta_pct"] > 2 for row in per_image),
        "median_time_regressions": sum(row["time_delta_pct"] > 0 for row in per_image),
        "per_image": per_image,
    }


def phase_summary(rows: list[dict[str, str]], profile: str) -> dict[str, object]:
    selected = [row for row in rows if row["profile"] == profile]
    total = lambda key: sum(int(row[key]) for row in selected)
    old_bytes = total("ordered_product_bytes")
    new_bytes = total("exact_product_bytes")
    old_ns = total("ordered_product_ns")
    new_ns = total("exact_product_ns")
    return {
        "images": len(selected),
        "pixels": total("pixels"),
        "tokens": total("tokens"),
        "blocks": total("blocks"),
        "signature_difference_blocks": total("signature_differences"),
        "assignment_difference_blocks": total("assignment_differences"),
        "ordered_product_bytes": old_bytes,
        "exact_product_bytes": new_bytes,
        "rate_delta_pct": (new_bytes / old_bytes - 1) * 100,
        "ordered_product_ns": old_ns,
        "exact_product_ns": new_ns,
        "speed_delta_pct": (new_ns / old_ns - 1) * 100,
        "ordered_candidate_ns": total("ordered_candidate_ns"),
        "exact_candidate_ns": total("exact_candidate_ns"),
        "ordered_plan_ns": total("ordered_plan_ns"),
        "counter_initialization_ns": total("initialization_ns"),
        "counter_update_ns": total("update_ns"),
        "signature_cluster_ns": total("signature_cluster_ns"),
        "counter_merge_ns": total("merge_ns"),
        "counter_updates": total("update_count"),
        "merge_updates": total("merge_updates"),
        "maximum_image_storage_bytes": max(int(row["storage_bytes"]) for row in selected),
    }


def phase_b_summary(rows: list[dict[str, str]], profile: str) -> dict[str, object]:
    selected = [row for row in rows if row["profile"] == profile]
    total = lambda key: sum(int(row[key]) for row in selected)
    old = total("ordered_product_bytes")
    exact = total("exact_product_bytes")
    coarse = total("coarse_product_bytes")
    return {
        "images": len(selected),
        "blocks": total("blocks"),
        "ordered_exact_signature_differences": total("ordered_exact_signature_differences"),
        "ordered_coarse_signature_differences": total("ordered_coarse_signature_differences"),
        "exact_coarse_signature_differences": total("exact_coarse_signature_differences"),
        "ordered_exact_assignment_differences": total("ordered_exact_assignment_differences"),
        "ordered_coarse_assignment_differences": total("ordered_coarse_assignment_differences"),
        "exact_coarse_assignment_differences": total("exact_coarse_assignment_differences"),
        "ordered_bytes": old,
        "exact_bytes": exact,
        "coarse_bytes": coarse,
        "exact_rate_delta_pct": (exact / old - 1) * 100,
        "coarse_rate_delta_pct": (coarse / old - 1) * 100,
        "exact_images_over_2pct": sum(int(row["exact_byte_delta"]) / int(row["ordered_product_bytes"]) > 0.02 for row in selected),
        "coarse_images_over_2pct": sum(int(row["coarse_byte_delta"]) / int(row["ordered_product_bytes"]) > 0.02 for row in selected),
        "exact_max_image_rate_delta_pct": max(int(row["exact_byte_delta"]) / int(row["ordered_product_bytes"]) * 100 for row in selected),
        "coarse_max_image_rate_delta_pct": max(int(row["coarse_byte_delta"]) / int(row["ordered_product_bytes"]) * 100 for row in selected),
        "exact_derivation_ns": total("exact_derivation_ns"),
        "coarse_derivation_ns": total("coarse_derivation_ns"),
        "exact_signature_cluster_ns": total("exact_signature_cluster_ns"),
        "coarse_signature_cluster_ns": total("coarse_signature_cluster_ns"),
    }


def main() -> None:
    root = Path(sys.argv[1] if len(sys.argv) > 1 else Path(__file__).parent)
    phase_a = tsv_rows(root / "raw/phase-a-102/phase-a.tsv", "phase_a")
    phase_b = tsv_rows(root / "raw/phase-b-102/phase-b.tsv", "phase_b")
    encode = process_data(root / "raw/screen-41-exact-symbol")
    rust_decode = process_data(root / "raw/screen-41-exact-symbol-rust-decode")
    c_decode = process_data(root / "raw/screen-41-exact-symbol-libwebp-decode")
    screen = {}
    for profile in ("compact", "low-latency"):
        control = f"{profile}-ordered-product-control"
        candidate = f"{profile}-frequency-owned"
        encoded = pair_summary(encode, control, candidate)
        rust = pair_summary(rust_decode, control, candidate)
        c = pair_summary(c_decode, control, candidate)
        encoded["screen_gate"] = (
            encoded["speed_delta_pct"] <= -10
            and encoded["rate_delta_pct"] <= 0.25
            and encoded["max_image_rate_delta_pct"] <= 2
            and encoded["median_time_regressions"] == 0
            and encoded["rss_delta_bytes"] <= 64 * 1024 * 1024
            and encoded["rss_delta_pct"] <= 5
            and rust["speed_delta_pct"] <= 1
            and c["speed_delta_pct"] <= 1
        )
        screen[profile] = {"encode": encoded, "rust_decode": rust, "libwebp_decode": c}
    b = {profile: phase_b_summary(phase_b, profile) for profile in ("compact", "low-latency")}
    summary = {
        "decision": "reject",
        "formal_102x5_run": False,
        "phase_a_exact_symbol": {profile: phase_summary(phase_a, profile) for profile in ("compact", "low-latency")},
        "exact_symbol_screen": screen,
        "phase_b_coarse_bin_mass": b,
        "phase_b_rate_prescreen": all(
            row["coarse_rate_delta_pct"] <= 0.25
            and row["coarse_images_over_2pct"] == 0
            and row["coarse_max_image_rate_delta_pct"] <= 2
            for row in b.values()
        ),
    }
    (root / "gate-summary.json").write_text(json.dumps(summary, indent=2, sort_keys=True) + "\n")


if __name__ == "__main__":
    main()
