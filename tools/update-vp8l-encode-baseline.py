#!/usr/bin/env python3
"""Record and compare durable VP8L encoder benchmark summaries."""

from __future__ import annotations

import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import platform
import shlex
import subprocess
import sys
from typing import Any


SCHEMA = 1
MEASUREMENT_CONTRACT = "vp8l-encode-e2e-preloaded-v1"
DATA_BEGIN = "<!-- BEGIN VP8L ENCODE BENCHMARK DATA"
DATA_END = "END VP8L ENCODE BENCHMARK DATA -->"
RUST_REFERENCE_LEVEL = {
    "default": 6,
    "high-compression": 9,
}


def command_output(*command: str, cwd: Path | None = None) -> str:
    return subprocess.check_output(
        command,
        cwd=cwd,
        stderr=subprocess.DEVNULL,
        text=True,
    ).strip()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def digest_paths(paths: list[Path], root: Path | None = None) -> str:
    digest = hashlib.sha256()
    for path in sorted(paths, key=lambda item: str(item)):
        label = str(path.relative_to(root)) if root is not None else str(path)
        digest.update(label.encode())
        digest.update(b"\0")
        digest.update(sha256_file(path).encode())
        digest.update(b"\n")
    return digest.hexdigest()


def read_inputs(path: Path) -> list[Path]:
    inputs = [Path(line) for line in path.read_text().splitlines() if line]
    if not inputs:
        raise SystemExit("benchmark input manifest is empty")
    missing = [str(item) for item in inputs if not item.is_file()]
    if missing:
        raise SystemExit(f"benchmark inputs are missing: {missing}")
    return inputs


def corpus_identity(inputs: list[Path]) -> str:
    digest = hashlib.sha256()
    for path in inputs:
        digest.update(path.name.encode())
        digest.update(b"\0")
        digest.update(sha256_file(path).encode())
        digest.update(b"\n")
    return digest.hexdigest()


def cpu_identity() -> str:
    if sys.platform == "darwin":
        try:
            value = command_output("sysctl", "-n", "machdep.cpu.brand_string")
            if value:
                return value
        except (OSError, subprocess.CalledProcessError):
            pass
    return platform.processor() or platform.machine()


def common_contract(root: Path, inputs: list[Path], iterations: int) -> dict[str, Any]:
    return {
        "measurement_contract": MEASUREMENT_CONTRACT,
        "corpus_sha256": corpus_identity(inputs),
        "files": len(inputs),
        "iterations": iterations,
        "host": platform.node(),
        "os": platform.platform(),
        "machine": platform.machine(),
        "cpu": cpu_identity(),
    }


def implementation_identity(root: Path) -> dict[str, Any]:
    names = command_output(
        "git",
        "ls-files",
        "--cached",
        "--others",
        "--exclude-standard",
        "--",
        "webp-rs",
        cwd=root,
    ).splitlines()
    sources = [
        root / name
        for name in names
        if name.endswith(".rs")
        or Path(name).name in {"Cargo.toml", "Cargo.lock", "build.rs"}
    ]
    return {
        "git_commit": command_output("git", "rev-parse", "HEAD", cwd=root),
        "worktree_dirty": bool(
            command_output("git", "status", "--short", "--", "webp-rs", cwd=root)
        ),
        "source_sha256": digest_paths(sources, root),
        "rustc": command_output("rustc", "--version"),
    }


def reference_identity(root: Path) -> dict[str, Any]:
    oracle = root / "third_party/oracle/libwebp"
    adapter = root / "tools/libwebp_vp8l_encode_bench.c"
    return {
        "libwebp_commit": command_output("git", "rev-parse", "HEAD", cwd=oracle),
        "adapter_sha256": sha256_file(adapter),
        "cc": command_output("cc", "--version").splitlines()[0],
        "exact": True,
        "levels": list(range(10)),
    }


def parse_result_line(line: str, iterations: int) -> dict[str, Any]:
    fields: dict[str, str] = {}
    for token in shlex.split(line):
        if "=" in token:
            key, value = token.split("=", 1)
            fields[key] = value
    required = {
        "encoder",
        "profile",
        "files",
        "encodes",
        "rgba_bytes",
        "output_bytes",
        "elapsed_ms",
        "checksum",
    }
    missing = sorted(required - fields.keys())
    if missing:
        raise SystemExit(f"benchmark result is missing {missing}: {line}")
    output_bytes = int(fields["output_bytes"])
    rgba_bytes = int(fields["rgba_bytes"])
    if output_bytes % iterations or rgba_bytes % iterations:
        raise SystemExit(f"benchmark totals do not divide by iterations: {line}")
    result: dict[str, Any] = {
        "encoder": fields["encoder"],
        "profile": fields["profile"],
        "files": int(fields["files"]),
        "encodes": int(fields["encodes"]),
        "iterations": iterations,
        "rgba_bytes_per_corpus": rgba_bytes // iterations,
        "output_bytes_per_corpus": output_bytes // iterations,
        "elapsed_ms_per_corpus": float(fields["elapsed_ms"]) / iterations,
        "checksum": fields["checksum"],
    }
    if "level" in fields:
        result["level"] = int(fields["level"])
    if "exact" in fields:
        result["exact"] = fields["exact"] == "1"
    return result


def parse_results(path: Path, iterations: int) -> list[dict[str, Any]]:
    lines = [line for line in path.read_text().splitlines() if line.strip()]
    if not lines:
        raise SystemExit("benchmark produced no results")
    return [parse_result_line(line, iterations) for line in lines]


def validate_results(
    results: list[dict[str, Any]],
    contract: dict[str, Any],
    encoder: str,
) -> None:
    for result in results:
        if result["encoder"] != encoder:
            raise SystemExit(f"expected {encoder} result, found {result['encoder']}")
        if result["files"] != contract["files"]:
            raise SystemExit("benchmark result file count does not match contract")
        if result["encodes"] != contract["files"] * contract["iterations"]:
            raise SystemExit("benchmark result encode count does not match contract")


def load_document(path: Path) -> dict[str, Any] | None:
    if not path.exists():
        return None
    text = path.read_text()
    start = text.find(DATA_BEGIN)
    end = text.find(DATA_END)
    if start < 0 and end < 0:
        return None
    if start < 0 or end < 0 or end <= start:
        raise SystemExit(f"{path} has a malformed machine-readable benchmark record")
    payload_start = text.find("\n", start) + 1
    return json.loads(text[payload_start:end].strip())


def percent_delta(candidate: float, baseline: float) -> float:
    if baseline == 0:
        return 0.0
    return 100.0 * (candidate / baseline - 1.0)


def format_delta(candidate: float, baseline: float) -> str:
    return f"{percent_delta(candidate, baseline):+.3f}%"


def result_map(results: list[dict[str, Any]]) -> dict[str, dict[str, Any]]:
    return {str(result["profile"]): result for result in results}


def render_document(data: dict[str, Any]) -> str:
    contract = data["contract"]
    reference = data["libwebp"]
    rust = data.get("rust")
    lines = [
        "# VP8L encode benchmark baseline",
        "",
        "This file contains the current accepted Rust result and the fixed pinned-libwebp",
        "reference for one measurement contract. Raw rounds, binaries, and generated",
        "streams are temporary and are deleted after each benchmark job.",
        "",
        "## Measurement contract",
        "",
        f"- Contract: `{contract['measurement_contract']}`",
        f"- Corpus: `{contract['corpus_sha256']}` ({contract['files']} files)",
        f"- Host: `{contract['host']}`",
        f"- OS: `{contract['os']}`",
        f"- CPU: `{contract['cpu']}`",
        f"- Iterations per measured job: {contract['iterations']}",
        "",
        "## Fixed pinned-libwebp reference",
        "",
        f"- Commit: `{reference['identity']['libwebp_commit']}`",
        f"- Adapter: `{reference['identity']['adapter_sha256']}`",
        f"- Compiler: `{reference['identity']['cc']}`",
        "- Contract: lossless, exact transparent RGB, single-threaded, preloaded RGBA",
        "",
        "| Level | Output bytes | Time / corpus |",
        "|---:|---:|---:|",
    ]
    by_level = {result["level"]: result for result in reference["results"]}
    for level in sorted(by_level):
        result = by_level[level]
        lines.append(
            f"| {level} | {result['output_bytes_per_corpus']:,} | "
            f"{result['elapsed_ms_per_corpus']:.3f} ms |"
        )

    lines.extend(["", "## Current accepted Rust result", ""])
    if rust is None:
        lines.append("No Rust baseline has been promoted for this contract.")
    else:
        identity = rust["identity"]
        lines.extend(
            [
                f"- Git commit: `{identity['git_commit']}`",
                f"- Source digest: `{identity['source_sha256']}`",
                f"- Dirty when measured: `{str(identity['worktree_dirty']).lower()}`",
                f"- Toolchain: `{identity['rustc']}`",
                f"- Recorded: `{rust['recorded_at']}`",
                "",
                "| Profile | Output bytes | Time / corpus |",
                "|---|---:|---:|",
            ]
        )
        rust_by_profile = result_map(rust["results"])
        for profile in ("default", "high-compression"):
            result = rust_by_profile[profile]
            lines.append(
                f"| {profile} | {result['output_bytes_per_corpus']:,} | "
                f"{result['elapsed_ms_per_corpus']:.3f} ms |"
            )

        lines.extend(
            [
                "",
                "## Current horizontal comparison",
                "",
                "| Rust profile | libwebp reference | Size gap | Time gap |",
                "|---|---:|---:|---:|",
            ]
        )
        for profile, level in RUST_REFERENCE_LEVEL.items():
            rust_result = rust_by_profile[profile]
            reference_result = by_level[level]
            lines.append(
                f"| {profile} | level {level} | "
                f"{format_delta(rust_result['output_bytes_per_corpus'], reference_result['output_bytes_per_corpus'])} | "
                f"{format_delta(rust_result['elapsed_ms_per_corpus'], reference_result['elapsed_ms_per_corpus'])} |"
            )

    lines.extend(
        [
            "",
            "## Machine-readable record",
            "",
            DATA_BEGIN,
            json.dumps(data, indent=2, sort_keys=True),
            DATA_END,
            "",
        ]
    )
    return "\n".join(lines)


def print_candidate_comparison(
    candidate: list[dict[str, Any]],
    document: dict[str, Any],
) -> None:
    candidate_by_profile = result_map(candidate)
    reference_by_level = {
        result["level"]: result for result in document["libwebp"]["results"]
    }
    baseline = document.get("rust")
    baseline_by_profile = (
        result_map(baseline["results"]) if baseline is not None else {}
    )
    print("candidate Rust result")
    for profile in ("default", "high-compression"):
        current = candidate_by_profile[profile]
        message = (
            f"  {profile}: bytes={current['output_bytes_per_corpus']} "
            f"time_ms={current['elapsed_ms_per_corpus']:.3f}"
        )
        if profile in baseline_by_profile:
            previous = baseline_by_profile[profile]
            message += (
                f" vs-rust-bytes={format_delta(current['output_bytes_per_corpus'], previous['output_bytes_per_corpus'])}"
                f" vs-rust-time={format_delta(current['elapsed_ms_per_corpus'], previous['elapsed_ms_per_corpus'])}"
            )
        reference = reference_by_level[RUST_REFERENCE_LEVEL[profile]]
        message += (
            f" vs-libwebp-level={RUST_REFERENCE_LEVEL[profile]}"
            f" size-gap={format_delta(current['output_bytes_per_corpus'], reference['output_bytes_per_corpus'])}"
            f" time-gap={format_delta(current['elapsed_ms_per_corpus'], reference['elapsed_ms_per_corpus'])}"
        )
        print(message)


def write_document(path: Path, data: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_suffix(path.suffix + f".tmp.{os.getpid()}")
    temporary.write_text(render_document(data))
    temporary.replace(path)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("mode", choices=("reference", "candidate"))
    parser.add_argument("--root", type=Path, required=True)
    parser.add_argument("--iterations", type=int, required=True)
    parser.add_argument("--inputs", type=Path, required=True)
    parser.add_argument("--results", type=Path, required=True)
    parser.add_argument("--document", type=Path, required=True)
    parser.add_argument("--promote", action="store_true")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    root = args.root.resolve()
    inputs = read_inputs(args.inputs)
    contract = common_contract(root, inputs, args.iterations)
    results = parse_results(args.results, args.iterations)
    existing = load_document(args.document)

    if args.mode == "reference":
        validate_results(results, contract, "libwebp")
        levels = sorted(result.get("level") for result in results)
        if levels != list(range(10)) or any(not result.get("exact") for result in results):
            raise SystemExit("libwebp reference must contain exact levels 0 through 9")
        data = {
            "schema": SCHEMA,
            "contract": contract,
            "libwebp": {
                "identity": reference_identity(root),
                "recorded_at": datetime.now(timezone.utc).isoformat(),
                "results": results,
            },
            "rust": None,
        }
        write_document(args.document, data)
        print(f"recorded fixed libwebp reference in {args.document}")
        return 0

    if existing is None or existing.get("libwebp") is None:
        raise SystemExit(
            "no fixed libwebp reference; run tools/benchmark-vp8l-reference.sh once"
        )
    if existing.get("schema") != SCHEMA or existing["contract"] != contract:
        raise SystemExit(
            "benchmark contract differs from the fixed reference; "
            "record a new libwebp reference before benchmarking Rust"
        )
    validate_results(results, contract, "rust")
    profiles = sorted(result["profile"] for result in results)
    if profiles != ["default", "high-compression"]:
        raise SystemExit("Rust result must contain default and high-compression profiles")
    print_candidate_comparison(results, existing)
    if args.promote:
        existing["rust"] = {
            "identity": implementation_identity(root),
            "recorded_at": datetime.now(timezone.utc).isoformat(),
            "results": results,
        }
        write_document(args.document, existing)
        print(f"promoted Rust result to {args.document}")
    else:
        print("fixed baseline unchanged; pass --promote to replace it")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
