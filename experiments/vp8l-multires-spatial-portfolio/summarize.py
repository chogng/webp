#!/usr/bin/env python3
"""Summarize locked P17 Phase A evidence and enforce its hard stop gate."""

from __future__ import annotations

import csv
import hashlib
import json
from pathlib import Path
import sys


E40 = {"compact": 599_398_064, "low_latency": 617_047_520}
MAX_COUNTER_BYTES = {"r128": 34_373_632, "r256": 17_186_816}
TIMING_FIELDS = (
    "counter_init_ns",
    "counter_update_ns",
    "e_proposal_ns",
    "b_proposal_ns",
    "e_cost_ns",
    "b_cost_ns",
    "reassignment_ns",
    "rebuild_ns",
    "refined_cost_ns",
    "candidate_selection_ns",
)


def percent(candidate: int, control: int) -> float:
    return (candidate / control - 1) * 100


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def read_rows(root: Path) -> list[dict[str, str]]:
    lines = [
        line[4:]
        for line in (root / "raw/phase-a-102/phase-a.tsv").read_text().splitlines()
        if line.startswith("p17\t")
    ]
    rows = list(csv.DictReader(lines, delimiter="\t"))
    if len(rows) != 102:
        raise RuntimeError(f"expected 102 Phase A rows, found {len(rows)}")
    return rows


def profile_summary(
    rows: list[dict[str, str]], control_field: str, output_field: str, kind_field: str
) -> dict[str, object]:
    control = sum(int(row[control_field]) for row in rows)
    output = sum(int(row[output_field]) for row in rows)
    per_image = [
        {
            "id": row["id"],
            "control_bytes": int(row[control_field]),
            "candidate_bytes": int(row[output_field]),
            "delta_pct": percent(int(row[output_field]), int(row[control_field])),
        }
        for row in rows
    ]
    worst = max(per_image, key=lambda row: float(row["delta_pct"]))
    kinds = sorted({row[kind_field] for row in rows})
    return {
        "control_bytes": control,
        "candidate_bytes": output,
        "delta_pct": percent(output, control),
        "worst_id": worst["id"],
        "worst_delta_pct": worst["delta_pct"],
        "images_over_2pct": sum(float(row["delta_pct"]) > 2 for row in per_image),
        "over_2pct_ids": [
            row["id"] for row in per_image if float(row["delta_pct"]) > 2
        ],
        "wins": {kind: sum(row[kind_field] == kind for row in rows) for kind in kinds},
        "per_image": per_image,
    }


def resolution_summary(rows: list[dict[str, str]], prefix: str) -> dict[str, object]:
    timing = {
        field.removesuffix("_ns"): sum(int(row[f"{prefix}_{field}"]) for row in rows)
        for field in TIMING_FIELDS
    }
    return {
        "blocks": sum(int(row[f"{prefix}_blocks"]) for row in rows),
        "counter_updates": sum(int(row[f"{prefix}_counter_updates"]) for row in rows),
        "merge_updates": {
            kind: sum(int(row[f"{prefix}_{kind}_merge_updates"]) for row in rows)
            for kind in ("e", "b", "refined")
        },
        "groups": {
            kind: sum(int(row[f"{prefix}_{kind}_groups"]) for row in rows)
            for kind in ("e", "b", "refined")
        },
        "proposal_wins": {
            kind: sum(row[f"{prefix}_winner"] == kind for row in rows)
            for kind in ("E", "B", "R")
        },
        "initial_wins": {
            kind: sum(row[f"{prefix}_initial"] == kind for row in rows)
            for kind in ("E", "B")
        },
        "timings_ns": timing,
        "maximum_measured_counter_bytes": max(
            int(row[f"{prefix}_counter_storage_bytes"]) for row in rows
        ),
        "maximum_measured_retained_plan_bytes": max(
            int(row[f"{prefix}_retained_plan_bytes"]) for row in rows
        ),
        "maximum_dimension_counter_bytes": MAX_COUNTER_BYTES[prefix],
    }


def artifact_summary(root: Path) -> dict[str, object]:
    with (root / "raw/binary-artifacts.tsv").open() as handle:
        rows = {row["name"]: row for row in csv.DictReader(handle, delimiter="\t")}
    result: dict[str, object] = {}
    for field in ("test_binary_bytes", "release_rlib_bytes"):
        base = int(rows["base"][field])
        candidate = int(rows["candidate"][field])
        result[field.removesuffix("_bytes")] = {
            "base_bytes": base,
            "candidate_bytes": candidate,
            "delta_bytes": candidate - base,
            "delta_pct": percent(candidate, base),
        }
    result["rows"] = rows
    return result


def main() -> None:
    root = Path(sys.argv[1] if len(sys.argv) > 1 else Path(__file__).parent)
    rows = read_rows(root)
    compact = profile_summary(rows, "compact_control_bytes", "compact_bytes", "compact_kind")
    low = profile_summary(rows, "low_control_bytes", "low_bytes", "low_kind")

    selector_exact = sum(
        row["selected_resolution"]
        == (
            "128"
            if int(row["r128_winner_riff_bytes"]) < int(row["r256_winner_riff_bytes"])
            else "256"
        )
        for row in rows
    )
    selected_128 = [row for row in rows if row["selected_resolution"] == "128"]
    selected_256 = [row for row in rows if row["selected_resolution"] == "256"]
    tails = {}
    for ident in ("clic-validation-008", "clic-validation-066", "clic-validation-074"):
        row = next(row for row in rows if row["id"] == ident)
        tails[ident] = {
            "control_128_bytes": int(row["compact_control_bytes"]),
            "control_256_bytes": int(row["low_control_bytes"]),
            "resolution_128_bytes": int(row["r128_winner_riff_bytes"]),
            "resolution_256_bytes": int(row["r256_winner_riff_bytes"]),
            "selected_resolution": row["selected_resolution"],
            "compact_bytes": int(row["compact_bytes"]),
            "low_bytes": int(row["low_bytes"]),
            "low_vs_e37_pct": percent(
                int(row["low_bytes"]), int(row["low_control_bytes"])
            ),
        }

    compact_gate = (
        int(compact["candidate_bytes"]) <= E40["compact"]
        and int(compact["images_over_2pct"]) == 0
    )
    low_gate = (
        int(low["candidate_bytes"]) <= E40["low_latency"]
        and int(low["images_over_2pct"]) == 0
    )
    summary = {
        "decision": "reject-phase-a" if not (compact_gate and low_gate) else "enter-screen",
        "formal_102x5_run": False,
        "screen_41_run": False,
        "manifest": {
            "rows": 102,
            "sha256": sha256(root / "raw/corpus-manifest-102.tsv"),
        },
        "exactness": {
            "resolution_rows": 204,
            "planned_stream_checks": 816,
            "resolution_selector_exact": selector_exact,
            "public_compact_exact": sum(int(row["public_compact_exact"]) for row in rows),
            "public_low_exact": sum(int(row["public_low_exact"]) for row in rows),
            "selected_128_matches_compact": sum(
                int(row["selected_128_matches_compact"]) for row in rows
            ),
        },
        "compact": compact,
        "low_latency": low,
        "e40_comparison": {
            "compact_bytes": E40["compact"],
            "compact_delta_bytes": int(compact["candidate_bytes"]) - E40["compact"],
            "low_latency_bytes": E40["low_latency"],
            "low_latency_delta_bytes": int(low["candidate_bytes"]) - E40["low_latency"],
            "low_latency_delta_pct": percent(
                int(low["candidate_bytes"]), E40["low_latency"]
            ),
        },
        "resolution_portfolio": {
            "selected_128": len(selected_128),
            "selected_256": len(selected_256),
            "selected_256_ids": [row["id"] for row in selected_256],
            "selected_128_compact_identical": sum(
                row["low_hash"] == row["compact_hash"] for row in selected_128
            ),
            "r128": resolution_summary(rows, "r128"),
            "r256": resolution_summary(rows, "r256"),
            "shared_prepare_ns": sum(int(row["shared_prepare_ns"]) for row in rows),
            "resolution_selection_ns": sum(
                int(row["resolution_selection_ns"]) for row in rows
            ),
            "single_selection_ns": sum(int(row["single_selection_ns"]) for row in rows),
        },
        "tails": tails,
        "memory": {
            "maximum_measured_accounted_live_bytes": max(
                int(row["maximum_accounted_live_bytes"]) for row in rows
            ),
            "maximum_dimension_counter_bytes": MAX_COUNTER_BYTES,
            "maximum_dimension_conservative_peak": "below 40 MiB; counters are sequential",
        },
        "artifacts": artifact_summary(root),
        "gates": {
            "compact_phase_a": compact_gate,
            "low_latency_phase_a": low_gate,
            "failure": "LowLatency image 074 is over the +2% E37 per-image limit",
        },
    }
    (root / "phase-a-summary.json").write_text(
        json.dumps(summary, indent=2, sort_keys=True) + "\n"
    )

    assert summary["manifest"]["sha256"] == (
        "9feb09f469753c43864011aa6f00cfc5ee1bd48da5aac8f9b16d105890e14f86"
    )
    assert selector_exact == 102
    assert summary["exactness"]["public_compact_exact"] == 102
    assert summary["exactness"]["public_low_exact"] == 102
    assert compact["candidate_bytes"] == E40["compact"]
    assert low["candidate_bytes"] == 599_169_200
    assert low["over_2pct_ids"] == ["clic-validation-074"]
    assert summary["decision"] == "reject-phase-a"


if __name__ == "__main__":
    main()
