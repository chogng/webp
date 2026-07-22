#!/usr/bin/env python3
import collections
import csv
import json
import pathlib
import statistics
import sys


ROOT = pathlib.Path(__file__).resolve().parent
RAW = ROOT / "raw" / "phase-a-102-final-screen-binary" / "phase-a.tsv"
E40 = {"compact": 599_398_064, "low-latency": 617_047_520}


def read_rows(path):
    headers = {}
    rows = collections.defaultdict(list)
    with path.open(newline="") as handle:
        for fields in csv.reader(handle, delimiter="\t"):
            if not fields:
                continue
            kind = fields[0]
            if kind not in {"p16", "p16-step"}:
                continue
            if len(fields) > 1 and fields[1] == "id":
                headers[kind] = fields[1:]
                continue
            if len(headers[kind]) != len(fields) - 1:
                raise ValueError(
                    f"{kind} field mismatch: {len(headers[kind])} != {len(fields) - 1}"
                )
            rows[kind].append(dict(zip(headers[kind], fields[1:])))
    return rows


def integer(row, key):
    return int(row[key])


def pct(value, baseline):
    return (value / baseline - 1.0) * 100.0


def summarize_profile(profile, rows, steps):
    selected = [row for row in rows if row["profile"] == profile]
    selected_steps = [row for row in steps if row["profile"] == profile]
    assert len(selected) == 102
    control = sum(integer(row, "control_riff_bytes") for row in selected)
    final = sum(integer(row, "final_riff_bytes") for row in selected)
    rates = [
        (
            pct(integer(row, "final_riff_bytes"), integer(row, "control_riff_bytes")),
            row["id"],
        )
        for row in selected
    ]
    worst_rate, worst_id = max(rates)
    wins = collections.Counter(row["final_kind"] for row in selected)
    attempts = len(selected_steps)
    accepted = sum(integer(row, "accepted") for row in selected_steps)
    monotone = all(
        integer(row, "candidate_riff_bytes") < integer(row, "before_riff_bytes")
        for row in selected_steps
        if integer(row, "accepted")
    )
    exact = {
        "planner_writer_plan_rows": len(selected) * 4,
        "public_output": sum(integer(row, "public_exact") for row in selected),
        "eb_selector": sum(integer(row, "eb_selector_exact") for row in selected),
        "final_selector": sum(integer(row, "final_selector_exact") for row in selected),
    }
    timing_keys = [
        "counter_init_ns",
        "counter_update_ns",
        "self_cost_ns",
        "e_proposal_ns",
        "b_proposal_ns",
        "e_cost_ns",
        "b_cost_ns",
        "p15_reassignment_ns",
        "p15_rebuild_ns",
        "refined_cost_ns",
        "selection_ns",
    ]
    step_timing_keys = [
        "regret_ns",
        "seed_ns",
        "partition_ns",
        "split_rebuild_ns",
        "reassignment_ns",
        "rebuild_ns",
        "exact_cost_ns",
    ]
    timings = {
        key: sum(integer(row, key) for row in selected) for key in timing_keys
    }
    timings.update(
        {
            "growth_" + key: sum(integer(row, key) for row in selected_steps)
            for key in step_timing_keys
        }
    )
    tails = {}
    for image_id in ["clic-validation-008", "clic-validation-066", "clic-validation-074"]:
        row = next(row for row in selected if row["id"] == image_id)
        image_steps = [step for step in selected_steps if step["id"] == image_id]
        tails[image_id] = {
            "control_bytes": integer(row, "control_riff_bytes"),
            "refined_bytes": integer(row, "refined_riff_bytes"),
            "split_bytes": integer(row, "split_riff_bytes"),
            "final_bytes": integer(row, "final_riff_bytes"),
            "final_vs_control_pct": pct(
                integer(row, "final_riff_bytes"), integer(row, "control_riff_bytes")
            ),
            "refined_groups": integer(row, "refined_groups"),
            "split_groups": integer(row, "split_groups"),
            "steps": image_steps,
        }
    return {
        "images": len(selected),
        "control_bytes": control,
        "e40_bytes": E40[profile],
        "final_bytes": final,
        "vs_control_pct": pct(final, control),
        "vs_e40_pct": pct(final, E40[profile]),
        "worst_image": worst_id,
        "worst_image_pct": worst_rate,
        "images_over_2pct": sum(rate > 2.0 for rate, _ in rates),
        "wins": dict(sorted(wins.items())),
        "growth_attempts": attempts,
        "growth_accepted": accepted,
        "accepted_steps_monotone": monotone,
        "exactness": exact,
        "timings_ns": timings,
        "counter_updates": sum(integer(row, "counter_updates") for row in selected),
        "max_counter_storage_bytes": max(
            integer(row, "counter_storage_bytes") for row in selected
        ),
        "max_retained_plan_bytes": max(
            integer(row, "retained_plan_bytes") for row in selected
        ),
        "max_blocks": max(integer(row, "blocks") for row in selected),
        "tails": tails,
    }


def process_samples(path):
    result = collections.defaultdict(list)
    for line in (path / "processes.jsonl").read_text().splitlines():
        process = json.loads(line)
        if process.get("phase") != "formal":
            continue
        output = pathlib.Path(process["stdout"])
        if not output.exists():
            output = path / "measurements" / output.name
        records = list(csv.reader(output.read_text().splitlines(), delimiter="\t"))
        aggregate = next(row for row in records if row and row[0] == "aggregate")
        items = {
            row[4]: {
                "ns": int(row[5]),
                "stream_bytes": int(row[7]),
                "hash": row[8],
            }
            for row in records
            if row and row[0] == "measurement"
        }
        result[process["layout"]].append(
            {
                "round": int(process["round"]),
                "aggregate_ns": int(aggregate[5]),
                "stream_bytes": int(aggregate[7]),
                "process_wall_ns": int(process["process_wall_ns"]),
                "cpu_ns": int(process["user_ns"]) + int(process["sys_ns"]),
                "max_rss_bytes": int(process["max_rss_bytes"]),
                "items": items,
            }
        )
    return result


def robust(values):
    median = statistics.median(values)
    mad = statistics.median(abs(value - median) for value in values)
    return {
        "samples": values,
        "median": median,
        "mad": mad,
        "outlier_3mad": [
            False if mad == 0 else abs(value - median) > 3 * mad for value in values
        ],
    }


def pair_summary(samples, control, candidate):
    controls = sorted(samples[control], key=lambda row: row["round"])
    candidates = sorted(samples[candidate], key=lambda row: row["round"])
    assert len(controls) == len(candidates) == 3
    control_ns = [row["aggregate_ns"] for row in controls]
    candidate_ns = [row["aggregate_ns"] for row in candidates]
    control_median = statistics.median(control_ns)
    candidate_median = statistics.median(candidate_ns)
    per_image = []
    for image_id in sorted(controls[0]["items"]):
        old = statistics.median(row["items"][image_id]["ns"] for row in controls)
        new = statistics.median(row["items"][image_id]["ns"] for row in candidates)
        old_bytes = controls[0]["items"][image_id]["stream_bytes"]
        new_bytes = candidates[0]["items"][image_id]["stream_bytes"]
        per_image.append(
            {
                "id": image_id,
                "control_median_ns": old,
                "candidate_median_ns": new,
                "time_delta_pct": pct(new, old),
                "control_bytes": old_bytes,
                "candidate_bytes": new_bytes,
                "rate_delta_pct": pct(new_bytes, old_bytes),
            }
        )
    control_rss = statistics.median(row["max_rss_bytes"] for row in controls)
    candidate_rss = statistics.median(row["max_rss_bytes"] for row in candidates)
    control_bytes = controls[0]["stream_bytes"]
    candidate_bytes = candidates[0]["stream_bytes"]
    return {
        "control": control,
        "candidate": candidate,
        "aggregate_control_ns": robust(control_ns),
        "aggregate_candidate_ns": robust(candidate_ns),
        "speed_delta_pct": pct(candidate_median, control_median),
        "paired_delta_pct": robust(
            [pct(new["aggregate_ns"], old["aggregate_ns"]) for old, new in zip(controls, candidates)]
        ),
        "control_process_wall_ns": robust([row["process_wall_ns"] for row in controls]),
        "candidate_process_wall_ns": robust([row["process_wall_ns"] for row in candidates]),
        "control_cpu_ns": robust([row["cpu_ns"] for row in controls]),
        "candidate_cpu_ns": robust([row["cpu_ns"] for row in candidates]),
        "control_rss_bytes": robust([row["max_rss_bytes"] for row in controls]),
        "candidate_rss_bytes": robust([row["max_rss_bytes"] for row in candidates]),
        "rss_delta_bytes": candidate_rss - control_rss,
        "rss_delta_pct": pct(candidate_rss, control_rss),
        "control_bytes": control_bytes,
        "candidate_bytes": candidate_bytes,
        "rate_delta_pct": pct(candidate_bytes, control_bytes),
        "max_image_rate_delta_pct": max(row["rate_delta_pct"] for row in per_image),
        "max_image_rate_id": max(per_image, key=lambda row: row["rate_delta_pct"])["id"],
        "images_rate_over_2pct": sum(row["rate_delta_pct"] > 2 for row in per_image),
        "median_time_regressions": sum(row["time_delta_pct"] > 0 for row in per_image),
        "per_image": per_image,
    }


def summarize_screen():
    paths = {
        "encode": ROOT / "raw/screen-41-encode-final",
        "rust_decode": ROOT / "raw/screen-41-rust-decode-final",
        "pinned_c_decode": ROOT / "raw/screen-41-libwebp-decode-final",
    }
    if not all(path.exists() for path in paths.values()):
        return None
    samples = {name: process_samples(path) for name, path in paths.items()}
    screen = {}
    for profile, control, candidate in [
        ("compact", "compact-control", "compact"),
        ("low-latency", "low-latency-control", "low-latency"),
    ]:
        result = {
            name: pair_summary(data, control, candidate)
            for name, data in samples.items()
        }
        encode = result["encode"]
        result["gates"] = {
            "encode_at_least_10pct_faster": encode["speed_delta_pct"] <= -10,
            "zero_per_image_encode_regressions": encode["median_time_regressions"] == 0,
            "aggregate_rate_not_worse_than_e37": encode["rate_delta_pct"] <= 0,
            "every_image_rate_within_plus_2pct": encode["images_rate_over_2pct"] == 0,
            "rust_decode_within_plus_1pct": result["rust_decode"]["speed_delta_pct"] <= 1,
            "pinned_c_decode_within_plus_1pct": result["pinned_c_decode"]["speed_delta_pct"] <= 1,
            "rss_delta_below_64mib": encode["rss_delta_bytes"] < 64 * 1024 * 1024,
            "rss_delta_below_5pct": encode["rss_delta_pct"] < 5,
        }
        result["screen_gate"] = all(result["gates"].values())
        screen[profile] = result
    project_rows = [
        line.split("\t")
        for line in (ROOT / "raw/screen-41-correctness-final/project-generate.tsv").read_text().splitlines()
        if line.startswith("stream\t") and not line.startswith("stream\tid\t")
    ]
    oracle = next(
        line
        for line in (ROOT / "raw/screen-41-correctness-final/libwebp-compare.tsv").read_text().splitlines()
        if line.startswith("oracle_summary\t")
    )
    binary = json.loads((paths["encode"] / "run.json").read_text())
    result = {
        "binary_sha256": binary["binary_sha256"],
        "binary_bytes": binary["binary_bytes"],
        "screen": screen,
        "correctness": {
            "project_streams": len(project_rows),
            "project_exact": sum(row[-1] == "1" for row in project_rows),
            "pinned_c_summary": oracle,
        },
        "screen_gate": all(profile["screen_gate"] for profile in screen.values())
        and len(project_rows) == 246
        and all(row[-1] == "1" for row in project_rows)
        and oracle == "oracle_summary\tmatched=246\tfailed=0",
        "formal_102x5_run": False,
    }
    result["decision"] = "pass-screen" if result["screen_gate"] else "reject-screen"
    (ROOT / "screen-summary.json").write_text(json.dumps(result, indent=2, sort_keys=True) + "\n")
    return result


def main():
    rows = read_rows(RAW)
    screen_run = ROOT / "raw/screen-41-encode-final/run.json"
    if screen_run.exists():
        binary = json.loads(screen_run.read_text())
        binary_sha256 = binary["binary_sha256"]
        binary_bytes = binary["binary_bytes"]
    else:
        binary_sha256 = "298abf0e1a02ba698367ef73c50b5347da737edac35d9456b76badda52b428ee"
        binary_bytes = 2_220_144
    summary = {
        "binary_sha256": binary_sha256,
        "binary_bytes": binary_bytes,
        "manifest_sha256": "9feb09f469753c43864011aa6f00cfc5ee1bd48da5aac8f9b16d105890e14f86",
        "profiles": {
            profile: summarize_profile(profile, rows["p16"], rows["p16-step"])
            for profile in ["compact", "low-latency"]
        },
    }
    compact = summary["profiles"]["compact"]
    low = summary["profiles"]["low-latency"]
    exact = all(
        profile["exactness"][key] == (408 if key == "planner_writer_plan_rows" else 102)
        for profile in [compact, low]
        for key in profile["exactness"]
    )
    tails_pass = all(
        low["tails"][image_id]["final_vs_control_pct"] <= 2.0
        for image_id in low["tails"]
    )
    aggregate_pass = all(
        summary["profiles"][profile]["final_bytes"] <= E40[profile]
        for profile in E40
    )
    summary["gate"] = {
        "exactness_pass": exact,
        "aggregate_not_worse_than_e40": aggregate_pass,
        "low_latency_tails_within_e37_plus_2pct": tails_pass,
        "phase_a_pass": exact and aggregate_pass and tails_pass,
    }
    (ROOT / "phase-a-summary.json").write_text(
        json.dumps(summary, indent=2, sort_keys=True) + "\n"
    )
    tail_lines = [
        "# P16 LowLatency tail evolution",
        "",
        "Every row is the single predeclared split candidate for that iteration. "
        "All deltas are complete RIFF bytes relative to the previous accepted plan.",
        "",
    ]
    for image_id, tail in low["tails"].items():
        tail_lines.extend(
            [
                f"## {image_id}",
                "",
                "| step | groups | source | seeds | regret | merge penalty | partition old/new | RIFF delta | accepted |",
                "| ---: | ---: | ---: | --- | ---: | ---: | ---: | ---: | ---: |",
            ]
        )
        for step in tail["steps"]:
            tail_lines.append(
                "| {iteration} | {before_groups}->{resulting_groups} | {source_group} | "
                "{seed_one}/{seed_two} | {regret} | {seed_penalty} | "
                "{old_partition_blocks}/{new_partition_blocks} | {riff_delta} | {accepted} |".format(
                    **step
                )
            )
        tail_lines.extend(
            [
                "",
                f"Final: refined groups {tail['refined_groups']}, split groups {tail['split_groups']}; "
                f"refined {tail['refined_bytes']:,} B, split/final {tail['final_bytes']:,} B, "
                f"E37 control {tail['control_bytes']:,} B ({tail['final_vs_control_pct']:+.6f}%).",
                "",
            ]
        )
    (ROOT / "TAILS.md").write_text("\n".join(tail_lines).rstrip() + "\n")
    screen = summarize_screen()
    if screen is not None:
        summary["screen"] = screen
        (ROOT / "gate-summary.json").write_text(
            json.dumps(summary, indent=2, sort_keys=True) + "\n"
        )
    print(json.dumps(summary["gate"], sort_keys=True))


if __name__ == "__main__":
    main()
