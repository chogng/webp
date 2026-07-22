# VP8L profile-aware streaming spatial plan (P13)

Date: 2026-07-22 (America/Los_Angeles).

## Decision

**Reject P13 and do not open a product migration.** The corrected strongest
streaming combination, S+C+F, improved the 41-image screen by only 2.658% for
Compact and 3.191% for LowLatency. Compact also regressed on 1/41 images. The
separate materialized-residual C+F diagnostic reached 5.899% and 3.520%, with
one Compact regression. Neither result met the predeclared requirement of at
least 10% independently for both profiles and zero image regressions.

The formal 102-image, five-round benchmark was therefore deliberately not
started. This is a gate-driven reject, not missing evidence. All three screens,
including the initial double-evaluation implementation, are retained.

## Provenance and branch discipline

- Creation-time base, local `main`, and merge-base:
  `cec68762e5ab6184bce275aeff5720ba3e6f96c7`.
- Branch: `codex/vp8l-streaming-spatial-plan`.
- Worktree: `/Users/lance/.codex/worktrees/25a6/webp`.
- S+C checkpoint: `daadb6f15268d3af6f0e2ae3198526cdf64aaf32`.
- F/four-way checkpoint: `f5e5bee5ba4a455da31678f90d89ac6d15368bae`.
- Single-evaluation correction: `815df5465b4266bd6e5cf1adbd3dcbb8e3b8c20c`.
- Final diagnostic/candidate code: `292c1d74cbc024207bf91c4a40d720c36190f0e2`.
- E37 packed-writer control checkpoint: `b3b96fdc27d2076b020b6d344f196e3ffc4cc6e1`.

After this worktree was created, `main` moved first to `180eafd4` for the P13
ledger entry and later to `e655ab9a` because of unrelated task ledger work.
This branch intentionally did not rebase. `origin/main` was not used.

Two helper task streams disconnected after their edits were on disk. No
benchmark was running. The interruption is recorded as task transport history,
not as a benchmark invalidation.

## Architecture and preserved invariants

S replaces the full residual `Vec<[u8; 4]>` with a single-pass pending-run
state machine. C updates each block's ordered Boyer-Moore census when a token is
emitted. F updates cache-0 exact per-block entropy counters at that same emit
point and merges blocks into assigned groups after deterministic clustering.

The following were proved unchanged by differential tests and complete output
identity:

- run segmentation: the first token of a run remains normal, copies remain
  capped at 4096, and one- or two-pixel tails remain literals;
- token order and color-cache effects;
- copy ownership by its starting pixel, including cross-row and cross-block
  copies;
- Boyer-Moore update order and zero-balance candidate retention;
- signature weighting, seed rank, assignment tie-breaking, empty-block fill,
  and group compaction;
- group frequencies, canonical Huffman tables, exact single fallback, strict
  candidate selection, profile wire parameters, and final bytes;
- Default, metadata, animation, API, and decoder behavior.

New named sibling tests cover runs 1/2/3, 4095/4096/4097 and repeated 4096
chunks, cross-row and 128/256 boundaries, tiny and transparent input, copy
ownership, exact block-frequency merge, every four-way pipeline byte result,
and the materialized C+F diagnostic.

## Phase A attribution

The benchmark-only Phase A path reconstructs E37 from materialized residuals,
times each responsibility separately, then compares its complete output with
production. It produced 102/102 byte-exact rows for both profiles. Timers and
census helpers are `cfg(test)` only.

Both profiles cover 251,858,137 pixels and 244,018,874 tokens:
242,507,972 literals, 1,510,902 copies, and zero cache tokens. Compact has
16,103 blocks; LowLatency has 4,216. The spatial candidate won on all 102
images in both profiles.

| phase (sum of per-image time) | Compact | LowLatency |
| --- | ---: | ---: |
| residual generation/materialization | 0.627167 s | 0.621843 s |
| tokenization/global frequencies | 0.912557 s | 0.901888 s |
| ordered block census | 2.504045 s | 2.530158 s |
| seed/rank/assign | 0.002743 s | 0.000964 s |
| group-frequency accumulation | 0.773295 s | 0.744966 s |
| single/map/group table build | 0.206255 s | 0.174008 s |
| packed writer | 3.276448 s | 3.179829 s |
| wrap/compare | 0.013765 s | 0.013263 s |

The old data path writes and rereads roughly 960.8 MiB of residual words,
then makes three full token reads after tokenization: census, group frequency,
and packed output. S targets the residual write/reread, C targets the census
token pass, and F replaces the group-frequency token pass with a bounded block
merge. The measured sum of the three nominally removable stages is 3.905 s
(Compact) and 3.897 s (LowLatency), an intentionally optimistic upper bound.
The screens demonstrate that producer-side statistics updates, dense counter
initialization/merge, and the remaining packed-writer scan make most of that
isolated upper bound unrecoverable in the integrated pipeline.

## Isolated 41-image screens

Every run used one full-repository archive release test binary, preloaded 41
fixed inputs, held `/private/tmp/webp-vp8l-product-benchmark.lock`, alternated
forward/reverse layout order, checksummed complete output, and retained every
sample plus `wait4` wall/CPU/max-RSS. The corpus manifest SHA-256 is
`9feb09f469753c43864011aa6f00cfc5ee1bd48da5aac8f9b16d105890e14f86`;
the screen manifest is
`474587feabe3178268b2eab6f7a166501d8ecc3d637a76bd412f4233dfa7b913`.

### Initial four-way implementation (`f5e5bee5`)

Binary SHA-256:
`9696a8aeb07ee31c2e6940ccc36ebfa01f7438f73ab364b5cba162c297b03dc4`.
This is valid failed variant evidence. It revealed that the initial S loop
evaluated the next nonmatching residual as lookahead and again as the next run
head.

| profile | variant | control | candidate | independent | regressions |
| --- | --- | ---: | ---: | ---: | ---: |
| Compact | S | 3.790389 s | 3.912878 s | +3.232% | 41/41 |
| Compact | S+C | 3.790389 s | 3.893262 s | +2.714% | 41/41 |
| Compact | S+C+F | 3.790389 s | 3.831007 s | +1.072% | 38/41 |
| LowLatency | S | 3.725384 s | 3.913674 s | +5.054% | 41/41 |
| LowLatency | S+C | 3.725384 s | 3.826446 s | +2.713% | 41/41 |
| LowLatency | S+C+F | 3.725384 s | 3.731333 s | +0.160% | 23/41 |

### Corrected four-way implementation (`815df546`)

Binary SHA-256:
`65bc7bf2ed40316cc7f12201bab56a5c5d812c359e4c2cd00b100b82d58b8e79`.
The corrected state machine evaluates each pixel residual exactly once.

| profile | variant | control | candidate | independent | regressions |
| --- | --- | ---: | ---: | ---: | ---: |
| Compact | S | 3.818069 s | 3.826793 s | +0.228% | 18/41 |
| Compact | S+C | 3.818069 s | 3.857032 s | +1.020% | 9/41 |
| Compact | S+C+F | 3.818069 s | 3.716571 s | **-2.658%** | 1/41 |
| LowLatency | S | 3.772696 s | 3.778202 s | +0.146% | 13/41 |
| LowLatency | S+C | 3.772696 s | 3.726026 s | -1.237% | 1/41 |
| LowLatency | S+C+F | 3.772696 s | 3.652312 s | **-3.191%** | 0/41 |

All output bytes matched. RSS deltas were favorable, from -3.33 to -3.81 MiB.
All samples and 3xMAD flags are retained in `gate-summary.json`; notably the
LowLatency S+C+F candidate's round 1 is retained.

### Materialized residual + synchronous C+F (`292c1d74`)

Binary SHA-256:
`311fc33097dd5075ce1f7a835db699dd0275165b534b0a157ed97d396b10cb76`.
This one predeclared diagnostic isolates C+F from S.

| profile | control | candidate | independent | paired median | regressions | RSS delta |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Compact | 4.015890 s | 3.778998 s | **-5.899%** | -3.515% | 1/41 | +0.484 MiB |
| LowLatency | 3.787675 s | 3.654337 s | **-3.520%** | -3.479% | 0/41 | +0.141 MiB |

Compact candidate round 2 and LowLatency candidate round 3 are retained 3xMAD
outliers. Even this strongest diagnostic is well below the 10% gate.

## Memory bounds

C stores an approximately 40-byte ordered census per block: at the maximum
16384x16384 image, at most 16,384 Compact blocks (0.625 MiB) or 4,096
LowLatency blocks (0.156 MiB).

F stores cache-0 symbols only: 280 green, three 256-entry channels, and one
copy counter, or 1,049 counters per block. Compact uses exact `u16` counters
because at most 128² token starts belong to a block; LowLatency uses `u32`
because 256² can equal 65,536. Worst-case F storage is 32.781 MiB for Compact
and 16.391 MiB for LowLatency; with C it is about 33.406 and 16.547 MiB.
Direct `EntropyFrequencies` per block was rejected because Compact would exceed
69 MiB before census overhead. The largest corpus image needs only 192/48
blocks, about 0.392/0.194 MiB for C+F.

Measured diagnostic RSS increased by only 507,904 bytes (0.087%) and 147,456
bytes (0.025%), passing the 64 MiB/5% memory gate. Memory was not the reason
for rejection.

## Correctness and product cost

Full-archive base/control/candidate builds completed 306/306
Default/Compact/LowLatency length, SHA-256, and full-byte identity rows. Each
archive's project decoder validated all 306 streams while generating them.
Pinned libwebp commit `733c91e461c18cf1127c9ed0a80dccbcfed599d3`
then matched 918/918 archive/profile RGBA decodes with zero failures. This also
proves candidate identity with the current E37 product.

The final candidate archive test binary SHA-256 is
`ecbfaab855063356198e8e8152f981a15dff4c25f54d14f6525e9e8c7b99219a`.

| artifact | E37 control | candidate | delta |
| --- | ---: | ---: | ---: |
| release rlib | 453,640 B | 471,712 B | +18,072 B (+3.98%) |
| release test binary | 1,501,056 B | 1,572,288 B | +71,232 B (+4.75%) |

The larger test-binary delta includes Phase A and all isolation layouts. F and
the materialized diagnostic are benchmark-only in the retained rejected tree;
the public path contains S+C. There are no new dependencies, threads, unsafe
blocks, public APIs, profile choices, or wire changes.

On stable Rust 1.97.1 for the host `aarch64-apple-darwin`, all of the following
passed from the candidate full archive:

- `cargo test --workspace --all-targets`;
- `cargo test --release --workspace --all-targets`;
- `cargo build --release --workspace --all-targets`;
- `cargo clippy --workspace --all-targets -- -D warnings`;
- `cargo fmt --all -- --check`;
- `RUSTDOCFLAGS='-D warnings' cargo doc -p webp --no-deps`;
- `cargo test -p webp --doc`.

No non-host target was used.

## Product assessment

The intended gains came from deleting the residual materialization/reload, the
independent block-census token pass, and the group-frequency token pass. The
data ownership and semantic invariants generalize cleanly, and the compact F
layout stays well inside the memory gate. Performance does not generalize:
integrated producer accounting and block merging recovered only a small part
of the isolated pass time.

E37 itself is 46.320%/46.406% faster than the historical E33 medians. Applying
the corrected screen ratios to E37 would project only 47.747%/48.116%
cumulative improvement, not more than 50%; this projection is not a formal
result. Even the materialized C+F diagnostic projects 49.487%/48.293%.

Therefore P13 is not worth a separate product migration. Do not merge this
research branch and do not open a latest-main migration tree. The useful
lasting result is the negative boundary: pass elimination alone is
insufficient unless future work removes the statistics-update/merge cost as
well, without changing token order, spatial ownership, or wire decisions.

## Evidence and reproduction

- `gate-summary.json`: Phase A and all retained screen samples/resources/gates.
- `invalidated-runs/INVALIDATED_RUNS.md`: interruption/setup audit.
- `reproduce.sh` and `summarize.py`: regeneration of per-image Phase A,
  all screens, identity/oracle rows, artifact/validation output, and checksums.

The generated `raw/` tree and `SHA256SUMS` stay in the selected reproduction
output directory and are intentionally not committed.

From the repository root, `experiments/vp8l-streaming-spatial-plan/reproduce.sh`
rebuilds every relevant full archive, reruns Phase A and all three screens,
asserts the reject gates, performs identity/oracle validation and quality
checks, and verifies a newly generated relative `SHA256SUMS`.

The script was executed from the repository root after evidence commit
`a2295c3df54f49aa2387fe8109f19bcf6b87fde8`; it completed with exit status 0
and verified every checksum in its fresh output directory.
