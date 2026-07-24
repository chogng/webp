#!/usr/bin/env python3
"""Run locked pinned-libwebp decode timing across VP8L product streams."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import signal
import sys
import time


LOCK = Path(__file__).resolve().parents[1] / "target/temporary/locks/webp-vp8l-libwebp-product-benchmark.lock"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--generated", type=Path, required=True)
    parser.add_argument("--expected", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--layouts", required=True)
    parser.add_argument("--rounds", type=int, default=5)
    parser.add_argument("--formal", action="store_true")
    return parser.parse_args()


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def run_process(
    binary: Path,
    expected: Path,
    generated: Path,
    layout: str,
    round_name: str,
    stdout: Path,
    stderr: Path,
) -> dict[str, object]:
    command = [
        str(binary.resolve()),
        round_name,
        layout,
        str(expected),
        str(generated / layout),
    ]
    started = time.monotonic_ns()
    pid = os.fork()
    if pid == 0:
        out = os.open(stdout, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o644)
        err = os.open(stderr, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o644)
        os.dup2(out, 1)
        os.dup2(err, 2)
        os.close(out)
        os.close(err)
        os.execv(command[0], command)
    waited, status, usage = os.wait4(pid, 0)
    if waited != pid:
        raise RuntimeError("wait4 returned the wrong child")
    record = {
        "command": command,
        "process_wall_ns": time.monotonic_ns() - started,
        "user_ns": int(usage.ru_utime * 1_000_000_000),
        "sys_ns": int(usage.ru_stime * 1_000_000_000),
        "max_rss_bytes": usage.ru_maxrss,
        "exit_status": os.waitstatus_to_exitcode(status),
        "stdout": str(stdout),
        "stderr": str(stderr),
    }
    if record["exit_status"] != 0:
        raise RuntimeError(f"benchmark failed: {record}")
    return record


def main() -> int:
    args = parse_args()
    if args.formal and args.rounds != 5:
        raise SystemExit("formal libwebp benchmark requires exactly five rounds")
    layouts = tuple(args.layouts.split(","))
    if not layouts or any(not layout for layout in layouts):
        raise SystemExit("at least one nonempty layout is required")
    args.output.mkdir(parents=True, exist_ok=False)
    measurements = args.output / "measurements"
    measurements.mkdir()
    LOCK.parent.mkdir(parents=True, exist_ok=True)
    descriptor = os.open(LOCK, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    os.write(descriptor, f"{os.getpid()}\n".encode())
    interrupted = False

    def stop(signum: int, _frame: object) -> None:
        nonlocal interrupted
        interrupted = True
        raise KeyboardInterrupt(f"signal {signum}")

    previous = {
        signum: signal.signal(signum, stop)
        for signum in (signal.SIGINT, signal.SIGTERM, signal.SIGHUP)
    }
    records: list[dict[str, object]] = []
    try:
        metadata = {
            "lock": str(LOCK),
            "lock_pid": os.getpid(),
            "started_unix_ns": time.time_ns(),
            "binary": str(args.binary.resolve()),
            "binary_sha256": sha256(args.binary),
            "binary_bytes": args.binary.stat().st_size,
            "decoder": "pinned libwebp WebPDecodeRGBA",
            "pinned_revision": "733c91e461c18cf1127c9ed0a80dccbcfed599d3",
            "rounds": args.rounds,
            "order": "forward on odd rounds, reverse on even rounds",
            "layouts": layouts,
            "operations": ["decode"],
            "baseline": "libwebp-m6",
            "formal": args.formal,
            "generated": str(args.generated),
            "expected": str(args.expected),
            "preloaded": True,
            "timed_validation": "full RGBA memcmp and full RGBA FNV-1a",
        }
        (args.output / "run.json").write_text(json.dumps(metadata, indent=2) + "\n")
        index = 0
        for layout in layouts:
            index += 1
            record = run_process(
                args.binary,
                args.expected,
                args.generated,
                layout,
                "warmup",
                measurements / f"{index:02d}-warmup-decode-{layout}.tsv",
                measurements / f"{index:02d}-warmup-decode-{layout}.stderr",
            )
            record.update(phase="warmup", operation="decode", layout=layout)
            records.append(record)
        for round_number in range(1, args.rounds + 1):
            order = layouts if round_number % 2 else tuple(reversed(layouts))
            for position, layout in enumerate(order, 1):
                index += 1
                record = run_process(
                    args.binary,
                    args.expected,
                    args.generated,
                    layout,
                    str(round_number),
                    measurements / f"{index:02d}-r{round_number}-decode-{layout}.tsv",
                    measurements / f"{index:02d}-r{round_number}-decode-{layout}.stderr",
                )
                record.update(
                    phase="formal",
                    operation="decode",
                    layout=layout,
                    round=round_number,
                    order="forward" if round_number % 2 else "reverse",
                    position=position,
                )
                records.append(record)
        metadata["completed_unix_ns"] = time.time_ns()
        (args.output / "run.json").write_text(json.dumps(metadata, indent=2) + "\n")
        with (args.output / "processes.jsonl").open("w") as output:
            for record in records:
                output.write(json.dumps(record, sort_keys=True) + "\n")
        return 0
    finally:
        for signum, handler in previous.items():
            signal.signal(signum, handler)
        os.close(descriptor)
        LOCK.unlink(missing_ok=True)
        if interrupted:
            print("libwebp benchmark interrupted; lock removed", file=sys.stderr)


if __name__ == "__main__":
    raise SystemExit(main())
