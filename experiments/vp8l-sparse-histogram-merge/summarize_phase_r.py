#!/usr/bin/env python3
"""Summarize and enforce the locked P21 mechanism and recovery gates."""

from __future__ import annotations

import csv
import hashlib
import json
import statistics
import sys
from collections import defaultdict
from pathlib import Path


def rows(path: Path, kind: str) -> list[list[str]]:
    with path.open(newline="") as stream:
        return [row for row in csv.reader(stream, delimiter="\t") if row and row[0] == kind]


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def provenance(path: Path) -> dict[str, str]:
    return dict(line.rstrip("\n").split("=", 1) for line in path.read_text().splitlines())


def main() -> None:
    output = Path(sys.argv[1]).resolve()
    raw = output / "raw"
    identity_rows = rows(raw / "phase-r-102.tsv", "phase-r")[1:]
    assert len(identity_rows) == 204

    p18: dict[tuple[str, str], tuple[int, str]] = {}
    for profile in ("compact", "low-latency"):
        for row in rows(raw / f"p18-{profile}.tsv", "measurement"):
            p18[(row[4], row[3])] = (int(row[7]), row[8])
    assert len(p18) == 204

    profile_rows: dict[str, list[list[str]]] = defaultdict(list)
    p18_matches = 0
    for row in identity_rows:
        profile_rows[row[2]].append(row)
        assert row[9:14] == ["1", "1", row[11], "1", "1"]
        assert row[13] == "1" and row[14] == "1"
        assert row[3] == row[5] == row[7]
        assert row[4] == row[6] == row[8]
        if (int(row[3]), row[4]) == p18[(row[1], row[2])]:
            p18_matches += 1
    assert p18_matches == 204

    expected_bytes = {"compact": 599_398_064, "low-latency": 601_400_998}
    expected_plans = {"compact": 306, "low-latency": 408}
    phase_profiles: dict[str, object] = {}
    for profile, profile_data in profile_rows.items():
        assert len(profile_data) == 102
        aggregate_bytes = sum(int(row[3]) for row in profile_data)
        plans = sum(int(row[11]) for row in profile_data)
        attempts = sum(int(row[16]) for row in profile_data)
        accepted = sum(int(row[17]) for row in profile_data)
        growth_state = sum(int(row[18]) for row in profile_data)
        assert aggregate_bytes == expected_bytes[profile]
        assert plans == expected_plans[profile]
        if profile == "compact":
            assert attempts == accepted == growth_state == 0
        else:
            assert attempts == accepted == 336 and growth_state == 102
        phase_profiles[profile] = {
            "images": 102,
            "aggregate_bytes": aggregate_bytes,
            "spatial_planner_writer_exact": f"{plans}/{expected_plans[profile]}",
            "growth": f"{accepted}/{attempts}",
            "growth_state_rows": growth_state,
        }

    census_rows = rows(raw / "census-102.tsv", "census")[1:]
    assert len(census_rows) == 1020
    census_totals: dict[tuple[str, str], list[int]] = defaultdict(lambda: [0, 0, 0])
    for row in census_rows:
        values = [int(row[4]), int(row[5]), int(row[6])]
        assert values[0] == values[1] + values[2]
        totals = census_totals[(row[2], row[3])]
        for index, value in enumerate(values):
            totals[index] += value
    census = {}
    for profile in ("compact", "low-latency"):
        census[profile] = {}
        for stage in ("self-model", "exact-symbol", "bin-mass", "refinement", "growth"):
            visits, nonzero, skipped = census_totals[(profile, stage)]
            census[profile][stage] = {
                "slot_visits": visits,
                "nonzero_adds": nonzero,
                "skipped_zero": skipped,
                "zero_elision_ratio": skipped / visits if visits else None,
            }

    prov = provenance(raw / "binary-provenance.txt")
    stderr_files = sorted(raw.glob("*.stderr"))
    stderr_bytes = sum(path.stat().st_size for path in stderr_files)
    assert stderr_bytes == 0
    phase_summary = {
        "task": prov["task"],
        "root_task": prov["root_task"],
        "branch": prov["branch"],
        "base": prov["base"],
        "head": prov["head"],
        "worktree": prov["worktree"],
        "binary_sha256": prov["p21_binary_sha256"],
        "p18_binary_sha256": prov["p18_binary_sha256"],
        "manifest_sha256": prov["corpus_manifest_sha256"],
        "a_b_identity": "204/204",
        "public_identity": "204/204",
        "p18_identity": "204/204",
        "selectors": {"eb": "204/204", "final": "204/204"},
        "strict_fallback": "204/204",
        "profiles": phase_profiles,
        "census": census,
        "stderr_bytes": stderr_bytes,
        "gate": True,
    }
    (output / "phase-r-summary.json").write_text(json.dumps(phase_summary, indent=2) + "\n")

    samples: dict[tuple[str, str, str], list[tuple[str, int, int, str]]] = defaultdict(list)
    aggregate_samples: dict[tuple[str, str], list[int]] = defaultdict(list)
    measured_files = sorted(raw.glob("[0-9][0-9]-r[123]-*.tsv"))
    assert len(measured_files) == 12
    for path in measured_files:
        measurements = rows(path, "measurement")
        aggregates = rows(path, "aggregate")
        assert len(measurements) == 41 and len(aggregates) == 1
        for row in measurements:
            samples[(row[3], row[4], row[5])].append((row[2], int(row[6]), int(row[8]), row[9]))
        aggregate = aggregates[0]
        aggregate_samples[(aggregate[3], aggregate[4])].append(int(aggregate[6]))

    recovery_profiles: dict[str, object] = {}
    identity = 0
    for profile in ("compact", "low-latency"):
        a_aggregates = aggregate_samples[(profile, "a")]
        b_aggregates = aggregate_samples[(profile, "b")]
        assert len(a_aggregates) == len(b_aggregates) == 3
        a_median = statistics.median(a_aggregates)
        b_median = statistics.median(b_aggregates)
        delta_pct = (b_median - a_median) * 100.0 / a_median
        regressions: list[tuple[str, float]] = []
        for image_id in sorted({key[2] for key in samples if key[0] == profile}):
            a = samples[(profile, "a", image_id)]
            b = samples[(profile, "b", image_id)]
            assert len(a) == len(b) == 3
            assert len({(item[2], item[3]) for item in a + b}) == 1
            identity += 1
            a_image = statistics.median(item[1] for item in a)
            b_image = statistics.median(item[1] for item in b)
            if b_image > a_image:
                regressions.append((image_id, (b_image - a_image) * 100.0 / a_image))
        gate = delta_pct <= 1.0 if profile == "compact" else delta_pct <= -3.0 and not regressions
        recovery_profiles[profile] = {
            "a_aggregate_ns": a_aggregates,
            "b_aggregate_ns": b_aggregates,
            "independent_delta_pct": delta_pct,
            "independent_improvement_pct": -delta_pct,
            "per_image_median_regressions": len(regressions),
            "regressions": [{"id": item[0], "delta_pct": item[1]} for item in regressions],
            "gate": gate,
        }
    assert identity == 82
    recovery_gate = all(item["gate"] for item in recovery_profiles.values())
    recovery_summary = {
        "task": prov["task"],
        "root_task": prov["root_task"],
        "branch": prov["branch"],
        "base": prov["base"],
        "head": prov["head"],
        "worktree": prov["worktree"],
        "binary_sha256": prov["p21_binary_sha256"],
        "manifest_sha256": prov["screen_manifest_sha256"],
        "round_order": "warmup then F/R/F",
        "retained_all_samples": True,
        "a_b_output_identity": "82/82",
        "profiles": recovery_profiles,
        "stderr_bytes": stderr_bytes,
        "gate": recovery_gate,
        "raw_sha256": {
            "phase_r": sha256(raw / "phase-r-102.tsv"),
            "census": sha256(raw / "census-102.tsv"),
            "p18_compact": sha256(raw / "p18-compact.tsv"),
            "p18_low_latency": sha256(raw / "p18-low-latency.tsv"),
        },
    }
    (output / "recovery-summary.json").write_text(json.dumps(recovery_summary, indent=2) + "\n")
    if not recovery_gate:
        raise SystemExit("locked recovery gate failed")


if __name__ == "__main__":
    main()
