# P14: VP8L frequency-owned spatial clustering

## Decision

**Reject P14 and do not open a product migration.** Exact per-block frequency
ownership proved the cost model: the exact-symbol candidate cleared the
41-image encode screen by 34.540% for Compact and 36.188% for LowLatency with
0/41 per-image median regressions. It nevertheless failed the predeclared rate
gate: aggregate RIFF bytes increased 0.423% and 0.388%, the worst images grew
5.841% and 5.058%, and 8/41 images in each profile exceeded +2%.

The one permitted follow-up, fixed coarse-bin-mass signature B, also failed its
102-image rate prescreen: +0.389% aggregate with 15/102 images over +2% for
Compact, and +0.419% with 14/102 over +2% for LowLatency. B therefore did not
run a screen. No third signature was tried. Because neither candidate passed
all hard gates, the formal 102x5 benchmark was deliberately not run.

## Provenance and commit chain

At task creation, all three required values were independently read and were
exactly equal:

- worktree HEAD: `3474599d89804cb91357788e967826544903011c`;
- local `main`: `3474599d89804cb91357788e967826544903011c`;
- `merge-base HEAD main`: `3474599d89804cb91357788e967826544903011c`.

The branch is `codex/vp8l-frequency-owned-clustering`, and the worktree is
`/Users/lance/.codex/worktrees/6d5d/webp`. `origin/main` was not used. Local
`main` later advanced to `1c16ebe826ea57adaf2293bf44bdc36175401a8b`; this
research branch intentionally retained its verified creation base.

Relevant commits:

- `c38e98aa431ed045774a0366a8c8d5d41d8eed46`: exact-frequency ownership and
  exact-symbol research hooks;
- `6703a1638c6015c0496457e8bf885ca3d079bd72`: symmetric exact-cost A/B paths;
- `be51b1e1`: initial Phase A evidence;
- `5c5099557618310d5edd1eb45353738a7e253152`: final exact-symbol archive
  harness;
- `9832274c`: exact-symbol Phase A, screen, correctness, and decoder evidence;
- `2d529c33e923df722ecd37d5964e9e89d46792bf`: the sole B/coarse-bin-mass
  checkpoint and final candidate code.
- `bb7002e9a877c3194b43fd3184a109df5aa70564`: complete decision, raw evidence,
  validation, and successful reproduction checkpoint.

The E37 packed-writer control is
`b3b96fdc27d2076b020b6d344f196e3ffc4cc6e1`. Pinned libwebp is
`733c91e461c18cf1127c9ed0a80dccbcfed599d3`. The host toolchain was stable
Rust 1.97.1 on `aarch64-apple-darwin`.

## Pre-benchmark A/B review

An independent review caught an asymmetric draft before any screen was run:
the ordered helper fully serialized a losing single stream, while the
frequency-owned production path used E35's exact-cost `SinglePlan` and skipped
that serialization when the spatial candidate won. That would have created a
false speedup. Commit `6703a163` corrected both explicit layouts to share
prepare, token/global frequencies, exact-cost `SinglePlan`, strict fallback,
candidate writer, and packed writer; only `SpatialPlan` construction differs.
Tests cover the common fallback and production/candidate identity. No headline
screen sample exists from the asymmetric draft.

## Architecture and invariants

Each spatial block is the sole owner of the exact counters needed by its final
entropy group. Compact blocks use `u16`, which is exact for at most 128 squared
token starts. LowLatency uses `u32`, because a full 256 squared block can have
65,536 starts. Each block stores 280 green counters, three 256-entry literal
channel counters, and one copy count: 1,049 counters total. Group frequencies
are produced by merging assigned blocks, so there is no second token scan.

E/exact-symbol derives each R/G/B/A signature component from the most frequent
exact symbol, resolving equal frequencies toward the lowest symbol, then maps
the symbol to its existing 32-symbol bin. B/coarse-bin-mass sums each channel's
exact counts into the same eight fixed 32-symbol bins and chooses the greatest
mass, resolving ties toward the lowest bin. B adds no token update and changes
only summary derivation.

Both variants preserve copy ownership by token start; token/global
frequencies; branch bins; seed weight and rank; assignment distance and seed
tie; group cap; empty-block fill and group compaction; map, Huffman table, and
packed-token writers; exact single fallback; public API; Default behavior; and
standard VP8L wire compatibility. There are no new dependencies, threads,
unsafe blocks, target requirements, or public APIs.

## Phase A: E/exact-symbol

The final archive release binary was built from `5c509955` and had SHA-256
`d8b037844510728b32a1ee29592001820cca5f973b2bc676c29bf2da1fc1f6fe`.
The locked 102-image replay covered 251,858,137 pixels and 244,018,874 tokens
per profile.

| metric | Compact | LowLatency |
| --- | ---: | ---: |
| blocks | 16,103 | 4,216 |
| signature differences vs ordered | 10,108 | 2,967 |
| assignment differences vs ordered | 12,673 | 3,424 |
| ordered product time | 7.693 s | 7.534 s |
| E product time | 4.742 s | 4.548 s |
| E speed delta | **-38.357%** | **-39.630%** |
| ordered plan time | 2.863 s | 2.855 s |
| counter initialization | 0.000432 s | 0.000227 s |
| counter update | 0.689942 s | 0.687216 s |
| signature/cluster | 0.033237 s | 0.008841 s |
| counter merge | 0.010180 s | 0.003154 s |
| exact counter updates | 973,053,692 | 973,053,692 |
| nonzero merge updates | 4,236,492 | 1,456,945 |
| aggregate bytes delta | +0.446% | +0.422% |

Candidate-only timing improved 47.821% and 49.965%. Thus the requested
question was answered positively: deleting four ordered Boyer-Moore updates
and deriving the signature from already-needed exact counters can comfortably
exceed 10% in both profiles. The same replay predicted a rate-gate failure,
which the independent screen then confirmed.

## E/exact-symbol 41-image screen

The fixed manifest hashes are
`9feb09f469753c43864011aa6f00cfc5ee1bd48da5aac8f9b16d105890e14f86`
for all 102 images and
`474587feabe3178268b2eab6f7a166501d8ecc3d637a76bd412f4233dfa7b913`
for the first 41. The screen used one final archive release test binary,
preloaded inputs, the global benchmark lock, forward/reverse alternation, three
rounds, complete-output checksums, and retained every sample and 3xMAD flag.

| screen gate | Compact | LowLatency |
| --- | ---: | ---: |
| control median encode | 3.541526 s | 3.501564 s |
| E median encode | 2.318275 s | 2.234421 s |
| encode delta | **-34.540%** | **-36.188%** |
| per-image median regressions | 0/41 | 0/41 |
| aggregate bytes | +0.423% | +0.388% |
| worst image bytes | +5.841% | +5.058% |
| images over +2% | 8/41 | 8/41 |
| median RSS delta | +688,128 B (+0.118%) | -11,550,720 B (-1.965%) |
| Rust decode delta | +0.273% | -2.215% |
| pinned C decode delta | -0.751% | +0.204% |
| complete gate | **fail: rate** | **fail: rate** |

Both decoders remained within the +1% gate. Rust decoder medians were
1.712209/1.716875 seconds for Compact control/E and
1.692343/1.654850 seconds for LowLatency. Pinned C medians were
2.208038/2.191464 and 2.186033/2.190483 seconds. The complete screen represents
41 images or 104.453 megapixels. Compact control/E encode throughput was
29.494/45.056 MP/s and rate was 2,476,976/2,487,459 bytes/MP; LowLatency was
29.830/46.747 MP/s and 2,505,727/2,515,449 bytes/MP. Rust decoder throughput
was 61.005/60.839 and 61.721/63.119 MP/s; pinned C was 47.306/47.664 and
47.782/47.685 MP/s.

## Phase B: coarse-bin-mass

B was predeclared only after E proved the cost model and failed rate. It reused
the identical exact counters and fixed eight 32-symbol bins; there was no
parameter search.

| 102-image rate prescreen | Compact | LowLatency |
| --- | ---: | ---: |
| ordered bytes | 617,958,802 | 625,321,072 |
| E bytes | 620,712,252 (+0.446%) | 627,958,884 (+0.422%) |
| B bytes | 620,360,862 (**+0.389%**) | 627,938,112 (**+0.419%**) |
| B worst image | +6.422% | +7.503% |
| B images over +2% | 15/102 | 14/102 |
| E/B signature differences | 4,825 | 1,488 |
| E/B assignment differences | 10,759 | 2,909 |
| E derivation time | 31.645 ms | 11.625 ms |
| B derivation time | 3.304 ms | 2.235 ms |

B made derivation cheaper and slightly improved aggregate Compact rate, but it
failed both original rate limits in both profiles. It therefore did not run a
41x3 screen. This is a gate-driven stop, not missing evidence.

## Memory and resources

At 16,384x16,384, Compact has at most 16,384 blocks and stores exactly
34,373,632 bytes (32.781 MiB) of `u16` counters. LowLatency has at most 4,096
blocks and stores 17,186,816 bytes (16.391 MiB) of `u32` counters. Assignments,
summaries, and at most 64/16 group-frequency records add well under 1 MiB, so
the static extra bound remains below +64 MiB. The largest corpus image used
402,816/201,408 counter bytes. Screen RSS was +0.118%/-1.965%, also below both
the +64 MiB and +5% gates.

Against E37, the final research archive's release rlib grew from 453,608 to
491,344 bytes (+37,736, +8.319%), and its release test binary grew from
1,501,056 to 1,556,912 bytes (+55,856, +3.721%). The test binary contains both
controls, both signature variants, and audit-only code; it is not a proposed
product artifact.

## Correctness and quality

The final three-archive verification covered creation base, E37, and P14-B.
All 102 Default streams were full-byte identical across all three archives.
Generation validated 918/918 Default/Compact/LowLatency streams with the
project Rust decoder, and pinned libwebp validated the same 918/918 streams
against complete RGBA with zero failures. Fast-profile byte identity was not
required and was not asserted.

Stable-host validation passed:

- `cargo test --workspace --all-targets`;
- `cargo test --release --workspace --all-targets`;
- `cargo build --release --workspace --all-targets`;
- `cargo clippy --workspace --all-targets -- -D warnings`;
- `cargo fmt --all -- --check`;
- `RUSTDOCFLAGS='-D warnings' cargo doc -p webp --no-deps`;
- `cargo test -p webp --doc`.

One initial rustdoc wrapper invocation split the environment value at its
space and returned 127 before rustdoc started. Its log is retained under
`invalidated-runs`; the correctly quoted command passed. No non-host target or
nightly toolchain was used.

## Product assessment and E33 cumulative result

Frequency ownership is architecturally sound and removes the dominant census
cost, but both tested deterministic signatures trade too much rate for the
predeclared product envelope. E's encode screen is strong enough that applying
its measured screen ratios to E37's historical E33 improvements would yield
64.861%/65.801% cumulative improvement. That is only a cross-experiment screen
calculation: because E failed rate and formal 102x5 was not run, P14 did **not**
formally demonstrate a greater-than-50% cumulative result relative to E33.
No projection is presented as a formal measurement.

Do not merge either research variant and do not create a latest-main product
migration. The durable result is narrower: exact block-frequency ownership is
fast and memory-safe, but a future proposal needs a substantially better fixed
assignment objective without regressing rate; P14 provides no authorization
for additional signature search.

## Evidence and reproduction

- `gate-summary.json`: machine-readable Phase A, screen, decode, MAD, and B
  prescreen results;
- `invalidated-runs/`: explanations of the incompatible identity helper and
  rustdoc wrapper error;
- `reproduce.sh` and `summarize.py`: one-command archive rebuild and
  regeneration of Phase A/B rows, screen/decoder samples, identity, validation,
  and checksums.

The complete row-level and process output, including its generated
`SHA256SUMS`, stays in the selected reproduction output directory rather than
the source repository.
