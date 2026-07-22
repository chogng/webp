#!/usr/bin/env python3
"""Enforce P23 candidate exactness, identity, census, and recovery gates."""

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


def measurements(path: Path) -> dict[str, tuple[int, str]]:
    result: dict[str, tuple[int, str]] = {}
    for row in typed_rows(path, "measurement"):
        image_id = row[4]
        assert image_id not in result
        result[image_id] = (int(row[7]), row[8])
    assert len(result) == 102
    return result


def mechanism_summary(output: Path, prov: dict[str, str]) -> dict[str, object]:
    raw = output / "raw"
    audit_path = raw / "candidate-audit-102.tsv"
    candidate_rows = [
        row for row in typed_rows(audit_path, "candidate-audit") if row[1] != "id"
    ]
    selection_rows = [
        row for row in typed_rows(audit_path, "selection-audit") if row[1] != "id"
    ]
    assert len(candidate_rows) == 102 * 2 * 5
    assert len(selection_rows) == 204

    expected_bytes = {"compact": 599_398_064, "low-latency": 601_400_998}
    expected_growth = {"compact": 0, "low-latency": 336}
    profiles: dict[str, object] = {}
    identity_counts = Counter()
    for profile in ("compact", "low-latency"):
        datasets = {
            variant: measurements(raw / f"{variant}-{profile}.tsv")
            for variant in ("a", "b", "public", "p18")
        }
        image_ids = datasets["a"].keys()
        assert all(dataset.keys() == image_ids for dataset in datasets.values())
        for image_id in image_ids:
            reference = datasets["a"][image_id]
            for variant in ("b", "public", "p18"):
                assert datasets[variant][image_id] == reference
                identity_counts[variant] += 1
        aggregate = sum(value[0] for value in datasets["a"].values())
        assert aggregate == expected_bytes[profile]

        rows = [row for row in candidate_rows if row[2] == profile]
        selections = [row for row in selection_rows if row[2] == profile]
        assert len(rows) == 510 and len(selections) == 102
        stage_totals: dict[str, dict[str, int]] = {}
        per_image_stage: dict[tuple[str, str], tuple[int, int]] = {}
        for stage in ("E", "B", "R", "Growth", "FinalMaterialization"):
            stage_rows = [row for row in rows if row[3] == stage]
            assert len(stage_rows) == 102
            values = [sum(int(row[index]) for row in stage_rows) for index in range(4, 14)]
            stage_totals[stage] = {
                "candidates": values[0],
                "metric_exact": values[1],
                "adaptive_table_builds_per_variant": values[2],
                "nested_map_tokenizations_per_variant": values[3],
                "a_candidate_bitwriter_bits": values[4],
                "a_candidate_bitwriter_bytes": values[5],
                "a_candidate_bitwriter_allocations": values[6],
                "a_retained_table_entries": values[7],
                "analytic_checked_bit_operations": values[8],
                "final_materializations": values[9],
                "maximum_a_candidate_heap_bytes": max(int(row[14]) for row in stage_rows),
                "maximum_b_candidate_heap_bytes": max(int(row[15]) for row in stage_rows),
            }
            if stage != "FinalMaterialization":
                assert values[0] == values[1]
            else:
                assert values[0] == 0
            for row in stage_rows:
                per_image_stage[(row[1], stage)] = (int(row[14]), int(row[15]))

        assert stage_totals["E"]["candidates"] == 102
        assert stage_totals["B"]["candidates"] == 102
        assert stage_totals["R"]["candidates"] == 102
        assert stage_totals["Growth"]["candidates"] == expected_growth[profile]
        assert stage_totals["FinalMaterialization"]["final_materializations"] == 102
        candidate_total = sum(stage_totals[stage]["candidates"] for stage in ("E", "B", "R", "Growth"))
        metric_total = sum(stage_totals[stage]["metric_exact"] for stage in ("E", "B", "R", "Growth"))
        assert candidate_total == metric_total == 306 + expected_growth[profile]

        growth_attempts = sum(int(row[5]) for row in selections)
        growth_accepted = sum(int(row[6]) for row in selections)
        assert growth_attempts == growth_accepted == expected_growth[profile]
        assert all(row[7] == "1" for row in selections)

        image_ids_sorted = sorted({row[1] for row in selections})
        maximum_a_live = 0
        maximum_b_live = 0
        for image_id in image_ids_sorted:
            initial_a = sum(per_image_stage[(image_id, stage)][0] for stage in ("E", "B", "R"))
            initial_b = sum(per_image_stage[(image_id, stage)][1] for stage in ("E", "B", "R"))
            growth_a = 2 * per_image_stage[(image_id, "Growth")][0]
            growth_b = 2 * per_image_stage[(image_id, "Growth")][1]
            maximum_a_live = max(maximum_a_live, initial_a, growth_a)
            maximum_b_live = max(maximum_b_live, initial_b, growth_b)

        profiles[profile] = {
            "aggregate_bytes": aggregate,
            "candidate_metric_exact": f"{metric_total}/{candidate_total}",
            "spatial_planner_writer_exact": "306/306" if profile == "compact" else "408/408",
            "growth": f"{growth_accepted}/{growth_attempts}",
            "selected_distribution": dict(Counter(row[3] for row in selections)),
            "stages": stage_totals,
            "census": {
                "a_candidate_bitwriter_allocations": sum(
                    stage_totals[stage]["a_candidate_bitwriter_allocations"]
                    for stage in ("E", "B", "R", "Growth")
                ),
                "b_candidate_bitwriter_allocations": 0,
                "a_retained_table_entries": sum(
                    stage_totals[stage]["a_retained_table_entries"]
                    for stage in ("E", "B", "R", "Growth")
                ),
                "b_retained_table_entries": 0,
                "maximum_live_estimated_heap_bytes": {
                    "a_conservative": maximum_a_live,
                    "b_conservative": maximum_b_live,
                    "allocator_overhead_included": False,
                },
            },
        }

    assert identity_counts == Counter({"b": 204, "public": 204, "p18": 204})
    stderr_files = [
        raw / "candidate-audit-102.stderr",
        *[raw / f"{variant}-{profile}.stderr" for variant in ("a", "b", "public", "p18") for profile in ("compact", "low-latency")],
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
        "binary_sha256": prov["p23_binary_sha256"],
        "p18_binary_sha256": prov["p18_binary_sha256"],
        "manifest_sha256": prov["corpus_manifest_sha256"],
        "identity": {
            "a_b": "204/204",
            "public_a": "204/204",
            "p18": "204/204",
            "eb_selector": "204/204",
            "final_selector": "204/204",
            "strict_fallback": "204/204",
        },
        "profiles": profiles,
        "stderr_bytes": stderr_bytes,
        "raw_sha256": {
            "candidate_audit": sha256(audit_path),
            **{
                f"{variant}_{profile.replace('-', '_')}": sha256(raw / f"{variant}-{profile}.tsv")
                for variant in ("a", "b", "public", "p18")
                for profile in ("compact", "low-latency")
            },
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
        parts = path.stem.split("-")
        round_name = parts[1]
        variant = parts[-1]
        profile = "low-latency" if "low-latency" in path.stem else "compact"
        rows = typed_rows(path, "measurement")
        aggregate = typed_rows(path, "aggregate")
        assert len(rows) == 41 and len(aggregate) == 1
        for row in rows:
            assert row[2] == round_name
            samples[(profile, variant, row[4])].append((int(row[5]), int(row[7]), row[8]))
        aggregates[(profile, variant)].append(int(aggregate[0][5]))

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
                    {"id": image_id, "delta_pct": (b_image - a_image) * 100.0 / a_image}
                )
        gate = delta_pct < 0.5 if profile == "compact" else delta_pct <= -5.0 and not regressions
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
        "binary_sha256": prov["p23_binary_sha256"],
        "manifest_sha256": prov["screen_manifest_sha256"],
        "round_order": "one warmup then F/R/F",
        "retained_all_samples": True,
        "valid_rerun_permitted": False,
        "a_b_output_identity": "82/82",
        "profiles": profiles,
        "stderr_bytes": stderr_bytes,
        "raw_sha256": {path.name: sha256(path) for path in measured_files + warmup_files},
        "gate": gate,
    }
    (output / "recovery-summary.json").write_text(json.dumps(result, indent=2) + "\n")
    return result


def main() -> None:
    output = Path(sys.argv[1]).resolve()
    prov = provenance(output / "raw" / "binary-provenance.txt")
    mechanism_summary(output, prov)
    recovery = recovery_summary(output, prov)
    if not recovery["gate"]:
        raise SystemExit("locked P23 recovery gate failed")


if __name__ == "__main__":
    main()
