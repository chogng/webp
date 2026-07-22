#!/usr/bin/env python3
"""Verify P25 A/O/B/P18 output identity from the locked Phase-R audit."""

from __future__ import annotations

import csv
import hashlib
import json
import sys
from pathlib import Path


def rows(path: Path) -> dict[str, tuple[int, str]]:
    result: dict[str, tuple[int, str]] = {}
    with path.open(newline="") as stream:
        for row in csv.reader(stream, delimiter="\t"):
            if row and row[0] == "measurement":
                result[row[4]] = (int(row[7]), row[8])
    assert len(result) == 102
    return result


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main() -> None:
    output = Path(sys.argv[1]).resolve()
    raw = output / "raw"
    expected = {"compact": 599_398_064, "low-latency": 601_400_998}
    profiles = {}
    for profile in expected:
        a = rows(raw / f"{profile}.tsv")
        o = rows(raw / f"{profile}-rank-sum.tsv")
        b = rows(raw / f"{profile}-fused-rank-sum.tsv")
        p18 = rows(raw / f"p18-{profile}.tsv")
        assert a == o == b == p18
        assert sum(value[0] for value in a.values()) == expected[profile]
        profiles[profile] = {
            "bytes": expected[profile],
            "a_o_b_p18_size_hash_identity": "102/102",
            "raw_sha256": {name: digest(raw / f"{name}.tsv") for name in [profile, f"{profile}-rank-sum", f"{profile}-fused-rank-sum", f"p18-{profile}"]},
        }
    stderr_bytes = sum(path.stat().st_size for path in raw.glob("*.stderr"))
    assert stderr_bytes == 0
    census_paths = sorted(raw.glob("candidate-census-shard-*.tsv"))
    assert len(census_paths) == 12
    census = []
    for path in census_paths:
        with path.open(newline="") as stream:
            census.extend(csv.reader(stream, delimiter="\t"))
    header = next(row for row in census if row and row[0] == "candidate-audit" and row[1] == "id")
    columns = {name: index for index, name in enumerate(header)}
    candidates = {"compact": 0, "low-latency": 0}
    o_tokens = {"compact": 0, "low-latency": 0}
    direct = {"compact": 0, "low-latency": 0}
    final_materializations = {"compact": 0, "low-latency": 0}
    stage_candidates = {(profile, stage): 0 for profile in candidates for stage in ["E", "B", "R", "Growth"]}
    audit_ids = {(profile, stage): set() for profile in candidates for stage in ["E", "B", "R", "Growth", "FinalMaterialization"]}
    for row in census:
        if not row or row[0] != "candidate-audit" or row[1] == "id":
            continue
        profile = row[columns["profile"]]
        stage = row[columns["stage"]]
        assert row[columns["id"]] not in audit_ids[profile, stage]
        audit_ids[profile, stage].add(row[columns["id"]])
        count = int(row[columns["candidates"]])
        candidates[profile] += count
        if stage != "FinalMaterialization":
            stage_candidates[profile, stage] += count
        o_tokens[profile] += int(row[columns["o_nested_map_tokenizations"]])
        direct[profile] += int(row[columns["b_direct_map_evaluations"]])
        final_materializations[profile] += int(row[columns["final_materializations"]])
        if row[columns["stage"]] == "FinalMaterialization":
            assert count == 0
            assert int(row[columns["metric_exact"]]) == int(row[columns["final_materializations"]])
        else:
            assert int(row[columns["metric_exact"]]) == count
        assert int(row[columns["b_nested_map_tokenizations"]]) == 0
        assert int(row[columns["b_nested_map_allocation_scopes_excluded"]]) == 0
        assert int(row[columns["b_adaptive_table_builds"]]) == 0
        assert int(row[columns["b_canonical_table_builds"]]) == 0
        assert int(row[columns["b_rank_sum_table_cost_heap_allocations"]]) == 0
        assert int(row[columns["b_direct_map_evaluations"]]) == count
        assert int(row[columns["o_nested_map_tokenizations"]]) == count
    assert candidates == {"compact": 306, "low-latency": 642}
    assert stage_candidates == {
        ("compact", "E"): 102,
        ("compact", "B"): 102,
        ("compact", "R"): 102,
        ("compact", "Growth"): 0,
        ("low-latency", "E"): 102,
        ("low-latency", "B"): 102,
        ("low-latency", "R"): 102,
        ("low-latency", "Growth"): 336,
    }
    assert o_tokens == candidates == direct
    assert final_materializations == {"compact": 102, "low-latency": 102}
    assert all(len(ids) == 102 for ids in audit_ids.values())
    selections = [row for row in census if row and row[0] == "selection-audit" and row[1] != "id"]
    assert len(selections) == 204
    selection_header = next(row for row in census if row and row[0] == "selection-audit" and row[1] == "id")
    selection_columns = {name: index for index, name in enumerate(selection_header)}
    growth = {"compact": [0, 0], "low-latency": [0, 0]}
    selection_ids = {"compact": set(), "low-latency": set()}
    for row in selections:
        profile = row[selection_columns["profile"]]
        assert row[selection_columns["id"]] not in selection_ids[profile]
        selection_ids[profile].add(row[selection_columns["id"]])
        assert row[selection_columns["strict_fallback_exact"]] == "1"
        growth[profile][0] += int(row[selection_columns["growth_attempts"]])
        growth[profile][1] += int(row[selection_columns["growth_accepted"]])
    assert growth == {"compact": [0, 0], "low-latency": [336, 336]}
    assert {profile: len(ids) for profile, ids in selection_ids.items()} == {"compact": 102, "low-latency": 102}
    provenance = dict(line.split("=", 1) for line in (raw / "provenance.txt").read_text().splitlines())
    hashes = dict(line.split("=", 1) for line in (raw / "binary-hashes.txt").read_text().splitlines())
    (output / "phase-r-summary.json").write_text(json.dumps({
        "task": "P25",
        "profiles": profiles,
        "identity": "204/204",
        "stderr_bytes": stderr_bytes,
        "measurement_head": provenance["measurement_head"],
        "p25_binary_sha256": hashes["p25_binary_sha256"],
        "p18_binary_sha256": hashes["p18_binary_sha256"],
        "gate": True,
    }, indent=2) + "\n")


if __name__ == "__main__":
    main()
