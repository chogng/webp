#!/usr/bin/env python3
"""Summarize P20 Phase A exactness, rate, tails, and P18 identity."""

from __future__ import annotations

import argparse
import json
from pathlib import Path


EXPECTED = {"compact": 599_398_064, "low-latency": 601_400_998}


def product_rows(path: Path) -> tuple[list[dict[str, object]], list[dict[str, object]]]:
    plans: list[dict[str, object]] = []
    images: list[dict[str, object]] = []
    for line in path.read_text().splitlines():
        fields = line.split("\t")
        if fields[0] == "plan" and fields[1] != "id":
            plans.append(
                {
                    "id": fields[1],
                    "profile": fields[2],
                    "kind": fields[3],
                    "predicted_bits": int(fields[4]),
                    "written_bits": int(fields[5]),
                    "predicted_payload_bytes": int(fields[6]),
                    "written_payload_bytes": int(fields[7]),
                    "predicted_riff_bytes": int(fields[8]),
                    "written_riff_bytes": int(fields[9]),
                    "stream_hash": fields[10],
                    "exact": fields[11] == "1",
                }
            )
        elif fields[0] == "phase-a" and fields[1] != "id":
            images.append(
                {
                    "id": fields[1],
                    "profile": fields[2],
                    "control_bytes": int(fields[3]),
                    "public_bytes": int(fields[4]),
                    "public_hash": fields[5],
                    "selected_kind": fields[6],
                    "final_kind": fields[7],
                    "growth_attempts": int(fields[8]),
                    "growth_accepted": int(fields[9]),
                    "has_growth_state": fields[10] == "1",
                    "eb_selector_exact": fields[11] == "1",
                    "final_selector_exact": fields[12] == "1",
                    "public_exact": fields[13] == "1",
                    "fallback_exact": fields[14] == "1",
                }
            )
    return plans, images


def p18_rows(path: Path) -> dict[str, dict[str, object]]:
    result: dict[str, dict[str, object]] = {}
    for line in path.read_text().splitlines():
        fields = line.split("\t")
        if fields[0] == "measurement" and fields[1] == "encode":
            result[fields[4]] = {
                "bytes": int(fields[7]),
                "stream_hash": fields[8],
            }
    return result


def profile_summary(
    profile: str,
    rows: list[dict[str, object]],
    oracle: dict[str, dict[str, object]],
) -> dict[str, object]:
    rows = sorted(rows, key=lambda row: str(row["id"]))
    identities = sum(
        oracle.get(str(row["id"]))
        == {"bytes": row["public_bytes"], "stream_hash": row["public_hash"]}
        for row in rows
    )
    ratios = [
        100 * (int(row["public_bytes"]) / int(row["control_bytes"]) - 1)
        for row in rows
    ]
    worst_index = max(range(len(rows)), key=lambda index: ratios[index])
    public_bytes = sum(int(row["public_bytes"]) for row in rows)
    control_bytes = sum(int(row["control_bytes"]) for row in rows)
    distribution: dict[str, int] = {}
    for row in rows:
        kind = str(row["final_kind"])
        distribution[kind] = distribution.get(kind, 0) + 1
    return {
        "images": len(rows),
        "p18_byte_identity": f"{identities}/{len(rows)}",
        "public_bytes": public_bytes,
        "expected_bytes": EXPECTED[profile],
        "control_bytes": control_bytes,
        "aggregate_rate_delta_pct": 100 * (public_bytes / control_bytes - 1),
        "worst_image": {
            "id": rows[worst_index]["id"],
            "delta_pct": ratios[worst_index],
        },
        "images_over_control_plus_2pct": sum(ratio > 2 for ratio in ratios),
        "growth_attempts": sum(int(row["growth_attempts"]) for row in rows),
        "growth_accepted": sum(int(row["growth_accepted"]) for row in rows),
        "growth_state_rows": sum(bool(row["has_growth_state"]) for row in rows),
        "eb_selectors_exact": sum(bool(row["eb_selector_exact"]) for row in rows),
        "final_selectors_exact": sum(bool(row["final_selector_exact"]) for row in rows),
        "public_selected_exact": sum(bool(row["public_exact"]) for row in rows),
        "strict_fallback_exact": sum(bool(row["fallback_exact"]) for row in rows),
        "final_distribution": distribution,
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--product", type=Path, required=True)
    parser.add_argument("--p18-compact", type=Path, required=True)
    parser.add_argument("--p18-low-latency", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()

    plans, images = product_rows(args.product)
    by_profile = {
        profile: [row for row in images if row["profile"] == profile]
        for profile in EXPECTED
    }
    summaries = {
        "compact": profile_summary(
            "compact", by_profile["compact"], p18_rows(args.p18_compact)
        ),
        "low-latency": profile_summary(
            "low-latency",
            by_profile["low-latency"],
            p18_rows(args.p18_low_latency),
        ),
    }
    compact_spatial = [
        row for row in plans if row["profile"] == "compact" and row["kind"] != "S"
    ]
    low_latency_spatial = [
        row
        for row in plans
        if row["profile"] == "low-latency" and row["kind"] != "S"
    ]
    single = [row for row in plans if row["kind"] == "S"]
    exactness = {
        "compact_spatial_planner_writer": f"{sum(bool(row['exact']) for row in compact_spatial)}/{len(compact_spatial)}",
        "low_latency_spatial_planner_writer": f"{sum(bool(row['exact']) for row in low_latency_spatial)}/{len(low_latency_spatial)}",
        "single_planner_writer": f"{sum(bool(row['exact']) for row in single)}/{len(single)}",
        "eb_selectors": f"{sum(int(summary['eb_selectors_exact']) for summary in summaries.values())}/204",
        "final_selectors": f"{sum(int(summary['final_selectors_exact']) for summary in summaries.values())}/204",
        "public_selected_streams": f"{sum(int(summary['public_selected_exact']) for summary in summaries.values())}/204",
        "strict_single_fallbacks": f"{sum(int(summary['strict_fallback_exact']) for summary in summaries.values())}/204",
        "p18_candidate_identity": "204/204"
        if all(summary["p18_byte_identity"] == "102/102" for summary in summaries.values())
        else "failed",
    }
    gate = (
        len(images) == 204
        and len(compact_spatial) == 306
        and len(low_latency_spatial) == 408
        and len(single) == 204
        and all(bool(row["exact"]) for row in plans)
        and summaries["compact"]["public_bytes"] == EXPECTED["compact"]
        and summaries["low-latency"]["public_bytes"] == EXPECTED["low-latency"]
        and all(summary["p18_byte_identity"] == "102/102" for summary in summaries.values())
        and all(int(summary["public_bytes"]) <= int(summary["control_bytes"]) for summary in summaries.values())
        and all(int(summary["images_over_control_plus_2pct"]) == 0 for summary in summaries.values())
        and summaries["compact"]["growth_attempts"] == 0
        and summaries["compact"]["growth_accepted"] == 0
        and summaries["compact"]["growth_state_rows"] == 0
        and summaries["low-latency"]["growth_attempts"] == 336
        and summaries["low-latency"]["growth_accepted"] == 336
        and summaries["low-latency"]["growth_state_rows"] == 102
        and all(int(summary["eb_selectors_exact"]) == 102 for summary in summaries.values())
        and all(int(summary["final_selectors_exact"]) == 102 for summary in summaries.values())
        and all(int(summary["public_selected_exact"]) == 102 for summary in summaries.values())
        and all(int(summary["strict_fallback_exact"]) == 102 for summary in summaries.values())
    )
    result = {
        "gate": gate,
        "profiles": summaries,
        "exactness": exactness,
        "plan_rows": len(plans),
        "image_profile_rows": len(images),
    }
    args.output.write_text(json.dumps(result, indent=2, sort_keys=True) + "\n")
    if not gate:
        raise SystemExit("Phase A gate failed")


if __name__ == "__main__":
    main()
