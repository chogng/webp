#!/usr/bin/env python3
"""Summarize and enforce P22 mechanism, census, identity, and recovery gates."""

from __future__ import annotations

import csv
import hashlib
import json
import statistics
import sys
from collections import Counter, defaultdict
from pathlib import Path


def typed_rows(path: Path, kind: str) -> list[list[str]]:
    with path.open(newline="") as stream:
        return [row for row in csv.reader(stream, delimiter="\t") if row and row[0] == kind]


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def provenance(path: Path) -> dict[str, str]:
    return dict(line.split("=", 1) for line in path.read_text().splitlines())


def p18_rows(raw: Path) -> dict[tuple[str, str], tuple[int, str]]:
    result: dict[tuple[str, str], tuple[int, str]] = {}
    for profile in ("compact", "low-latency"):
        for row in typed_rows(raw / f"p18-{profile}.tsv", "measurement"):
            result[(profile, row[4])] = (int(row[7]), row[8])
    assert len(result) == 204
    return result


def phase_summary(output: Path, prov: dict[str, str]) -> dict[str, object]:
    raw = output / "raw"
    rows = typed_rows(raw / "phase-r-102.tsv", "phase-r")[1:]
    assert len(rows) == 204
    p18 = p18_rows(raw)
    expected_bytes = {"compact": 599_398_064, "low-latency": 601_400_998}
    expected_plans = {"compact": 306, "low-latency": 408}
    profiles: dict[str, object] = {}
    p18_identity = 0
    for profile in ("compact", "low-latency"):
        profile_rows = [row for row in rows if row[2] == profile]
        assert len(profile_rows) == 102
        for row in profile_rows:
            assert row[3] == row[5] == row[7]
            assert row[4] == row[6] == row[8]
            assert row[12:18] == ["1"] * 6
            if (int(row[3]), row[4]) == p18[(profile, row[1])]:
                p18_identity += 1
        aggregate = sum(int(row[3]) for row in profile_rows)
        planner_rows = sum(int(row[11]) for row in profile_rows)
        attempts = sum(int(row[18]) for row in profile_rows)
        accepted = sum(int(row[19]) for row in profile_rows)
        assert aggregate == expected_bytes[profile]
        assert planner_rows == expected_plans[profile]
        if profile == "compact":
            assert attempts == accepted == 0
        else:
            assert attempts == accepted == 336
        profiles[profile] = {
            "images": 102,
            "aggregate_bytes": aggregate,
            "spatial_planner_writer_exact": f"{planner_rows}/{expected_plans[profile]}",
            "single_planner_writer_exact": "102/102",
            "growth": f"{accepted}/{attempts}",
            "selected_distribution": dict(Counter(row[9] for row in profile_rows)),
            "final_distribution": dict(Counter(row[10] for row in profile_rows)),
        }
    assert p18_identity == 204

    census_rows = typed_rows(raw / "census-102.tsv", "census")[1:]
    assert len(census_rows) == 2040
    stage_totals: dict[tuple[str, str, str], list[int]] = defaultdict(lambda: [0] * 6)
    high_water: dict[tuple[str, str], list[int]] = defaultdict(lambda: [0] * 4)
    census_streams: dict[tuple[str, str, str], tuple[int, str]] = {}
    for row in census_rows:
        key = (row[2], row[3], row[4])
        values = [int(value) for value in row[5:11]]
        totals = stage_totals[key]
        for index, value in enumerate(values):
            totals[index] += value
        maxima = high_water[(row[2], row[3])]
        for index, value in enumerate(row[11:15]):
            maxima[index] = max(maxima[index], int(value))
        stream_key = (row[2], row[3], row[1])
        stream = (int(row[15]), row[16])
        if stream_key in census_streams:
            assert census_streams[stream_key] == stream
        census_streams[stream_key] = stream
    assert len(census_streams) == 408
    for profile in ("compact", "low-latency"):
        for image_id in {key[2] for key in census_streams if key[0] == profile}:
            assert census_streams[(profile, "a", image_id)] == census_streams[
                (profile, "b", image_id)
            ]

    census: dict[str, object] = {}
    for profile in ("compact", "low-latency"):
        census[profile] = {}
        for variant in ("a", "b"):
            stages = {}
            for stage in ("E", "B", "R", "Growth", "FinalMaterialization"):
                values = stage_totals[(profile, variant, stage)]
                stages[stage] = {
                    "full_plans_built": values[0],
                    "final_materializations": values[1],
                    "plan_parts_clones": values[2],
                    "prefix_bytes": values[3],
                    "table_entries": values[4],
                    "estimated_plan_heap_bytes": values[5],
                }
                assert values[2] == 0
            if variant == "a":
                assert stages["FinalMaterialization"]["full_plans_built"] == 0
                assert stages["FinalMaterialization"]["final_materializations"] == 0
            else:
                assert stages["FinalMaterialization"]["full_plans_built"] == 102
                assert stages["FinalMaterialization"]["final_materializations"] == 102
            maximum = high_water[(profile, variant)]
            assert maximum[0] == (3 if variant == "a" else 1)
            census[profile][variant] = {
                "stages": stages,
                "maximum_live_full_plans": maximum[0],
                "maximum_live_tables": maximum[1],
                "maximum_live_prefix_bytes": maximum[2],
                "maximum_live_estimated_heap_bytes": maximum[3],
            }

    stderr_files = [
        raw / "phase-r-102.stderr",
        raw / "census-102.stderr",
        raw / "p18-compact.stderr",
        raw / "p18-low-latency.stderr",
    ]
    stderr_bytes = sum(path.stat().st_size for path in stderr_files)
    assert stderr_bytes == 0
    result = {
        "task": prov["task"],
        "root_task": prov["root_task"],
        "branch": prov["branch"],
        "base": prov["base"],
        "measurement_head": prov["head"],
        "worktree": prov["worktree"],
        "binary_sha256": prov["p22_binary_sha256"],
        "p18_binary_sha256": prov["p18_binary_sha256"],
        "manifest_sha256": prov["corpus_manifest_sha256"],
        "identity": {
            "a_b": "204/204",
            "public": "204/204",
            "p18": "204/204",
            "eb_selector": "204/204",
            "final_selector": "204/204",
            "strict_fallback": "204/204",
        },
        "profiles": profiles,
        "census": census,
        "stderr_bytes": stderr_bytes,
        "raw_sha256": {
            "phase_r": sha256(raw / "phase-r-102.tsv"),
            "census": sha256(raw / "census-102.tsv"),
            "p18_compact": sha256(raw / "p18-compact.tsv"),
            "p18_low_latency": sha256(raw / "p18-low-latency.tsv"),
        },
        "gate": True,
    }
    (output / "phase-r-summary.json").write_text(json.dumps(result, indent=2) + "\n")
    return result


def recovery_summary(output: Path, prov: dict[str, str]) -> dict[str, object]:
    raw = output / "raw"
    measured_files = sorted(raw.glob("[0-9][0-9]-r[123]-*.tsv"))
    warmup_files = sorted(raw.glob("[0-9][0-9]-warmup-*.tsv"))
    assert len(measured_files) == 12
    assert len(warmup_files) == 4
    samples: dict[tuple[str, str, str], list[tuple[int, int, str]]] = defaultdict(list)
    aggregates: dict[tuple[str, str], list[int]] = defaultdict(list)
    for path in measured_files:
        measurements = typed_rows(path, "metric-measurement")
        aggregate = typed_rows(path, "metric-aggregate")
        assert len(measurements) == 41 and len(aggregate) == 1
        for row in measurements:
            samples[(row[2], row[3], row[4])].append((int(row[5]), int(row[7]), row[8]))
        row = aggregate[0]
        aggregates[(row[2], row[3])].append(int(row[5]))

    profiles: dict[str, object] = {}
    identity = 0
    for profile in ("compact", "low-latency"):
        a_aggregates = aggregates[(profile, "a")]
        b_aggregates = aggregates[(profile, "b")]
        assert len(a_aggregates) == len(b_aggregates) == 3
        a_median = statistics.median(a_aggregates)
        b_median = statistics.median(b_aggregates)
        delta_pct = (b_median - a_median) * 100.0 / a_median
        regressions: list[dict[str, object]] = []
        image_ids = sorted({key[2] for key in samples if key[0] == profile})
        assert len(image_ids) == 41
        for image_id in image_ids:
            a = samples[(profile, "a", image_id)]
            b = samples[(profile, "b", image_id)]
            assert len(a) == len(b) == 3
            assert len({(item[1], item[2]) for item in a + b}) == 1
            identity += 1
            a_image = statistics.median(item[0] for item in a)
            b_image = statistics.median(item[0] for item in b)
            if b_image > a_image:
                regressions.append(
                    {
                        "id": image_id,
                        "delta_pct": (b_image - a_image) * 100.0 / a_image,
                    }
                )
        gate = delta_pct <= 1.0 if profile == "compact" else delta_pct <= -3.0 and not regressions
        profiles[profile] = {
            "a_aggregate_ns": a_aggregates,
            "b_aggregate_ns": b_aggregates,
            "a_median_ns": a_median,
            "b_median_ns": b_median,
            "independent_delta_pct": delta_pct,
            "independent_improvement_pct": -delta_pct,
            "per_image_median_regressions": len(regressions),
            "regressions": regressions,
            "gate": gate,
        }
    assert identity == 82
    stderr_files = sorted(raw.glob("[0-9][0-9]-*.stderr"))
    stderr_bytes = sum(path.stat().st_size for path in stderr_files)
    gate = all(bool(profile["gate"]) for profile in profiles.values()) and stderr_bytes == 0
    result = {
        "task": prov["task"],
        "root_task": prov["root_task"],
        "branch": prov["branch"],
        "base": prov["base"],
        "measurement_head": prov["head"],
        "worktree": prov["worktree"],
        "binary_sha256": prov["p22_binary_sha256"],
        "manifest_sha256": prov["screen_manifest_sha256"],
        "round_order": "one warmup then F/R/F",
        "retained_all_samples": True,
        "valid_rerun_permitted": False,
        "a_b_output_identity": "82/82",
        "profiles": profiles,
        "stderr_bytes": stderr_bytes,
        "gate": gate,
        "raw_sha256": {
            path.name: sha256(path)
            for path in measured_files + warmup_files
        },
    }
    (output / "recovery-summary.json").write_text(json.dumps(result, indent=2) + "\n")
    return result


def main() -> None:
    output = Path(sys.argv[1]).resolve()
    prov = provenance(output / "raw" / "binary-provenance.txt")
    phase_summary(output, prov)
    recovery = recovery_summary(output, prov)
    if not recovery["gate"]:
        raise SystemExit("locked P22 recovery gate failed")


if __name__ == "__main__":
    main()
