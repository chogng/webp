# Quality gates for codec milestones

Correct decoding is necessary but not sufficient for a milestone to be
complete. Every codec milestone must meet the following gates before being
labelled complete.

## Required gates

1. **Conformance and compatibility.** Public APIs must pass their pinned
   corpus fixtures and canonical output checks.
2. **Robustness.** Bounds, resource limits, malformed inputs, and relevant
   fuzz targets must pass. New parsing or entropy paths require a direct fuzz
   target or a documented reason why an existing target covers them.
3. **Performance.** The changed public path must have a repeatable release
   benchmark using representative local data. It must record the command,
   corpus identity, work performed, elapsed time, and throughput. A change may
   not regress its established median by more than 5% without an explicit,
   reviewed justification.
4. **Resource behaviour.** The benchmark/profile report must identify output
   allocation, peak retained working data, and the top CPU or allocation
   hotspots. New limits must be exercised by tests.

When a pinned oracle exists, the performance report must also include an
in-process comparison using equivalent decode work. Matching the oracle's
throughput is not an automatic requirement, but any material gap must have an
owner and a remediation plan. A milestone with an unprofiled material gap is
**performance pending**, not complete.

## VP8L M1 baseline

Run the VP8L comparison after fetching the pinned corpus and oracle:

```sh
bash tools/benchmark-vp8l.sh 5
```

The script selects every `MustAccept` VP8L decode fixture, preloads its input,
and reports the public Rust decoder and libwebp's `WebPDecodeRGBA` C API over
the same work. The native helper and Rust runner both include RGBA allocation
and pixel reconstruction but exclude input file I/O and process launch.

The local 2026-07-20 baseline covers 41 files (14.4 MB compressed) and 205
decodes / 114.8 MB RGBA per run. Across three five-iteration runs, the median
was 1.894 s (60.6 MB/s) for Rust and 0.518 s (221.4 MB/s) for libwebp: a 3.65x
gap. M1 is therefore functionally complete but performance pending.

## Applying the gates to later milestones

M0 owns the reusable fixture, corpus-pin, and benchmark infrastructure. Each
codec milestone owns its path-specific benchmark and profile. M2 and later
milestones must establish their performance baseline when the first public
decode path lands, then run it before claiming milestone completion.
