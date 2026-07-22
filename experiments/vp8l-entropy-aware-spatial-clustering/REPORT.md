# P15: exact-cost multi-proposal and entropy-aware spatial clustering

## Decision

**Reject P15 and do not open a product migration.** Phase A passed its
aggregate-rate prerequisite and the 41-image screen comfortably passed both
encode gates, but LowLatency failed two hard screen gates: image 008 grew
4.338207% (limit +2%), and aggregate Rust decode median regressed 1.179388%
(limit +1%). Formal 102x5 was therefore deliberately not run. The research
implementation and all negative evidence remain on this branch; none of it
should be merged into `main` as product code.

## Provenance

The worktree was created detached from the then-latest local `main`. Before any
modification, all three independently read values were exactly equal:

- worktree HEAD: `0e91e379aef2cfac1189472a3dd0627060f892b8`;
- local `main`: `0e91e379aef2cfac1189472a3dd0627060f892b8`;
- `merge-base HEAD main`: `0e91e379aef2cfac1189472a3dd0627060f892b8`.

The branch is `codex/vp8l-entropy-aware-spatial-clustering` and the worktree is
`/Users/lance/.codex/worktrees/3cd9/webp`. The branch was not rebased when P15
was later registered on main by `cef04c68`; that registration SHA is recorded
separately from the creation base. Relevant checkpoints are implementation
`eacad8bf`, Phase A harness/model correction `d23c7a1e`, Phase A evidence and
fair control `f78ca14e`, and final same-binary screen generator `7d14b835`.
Pinned libwebp is `733c91e461c18cf1127c9ed0a80dccbcfed599d3`.

## Architecture and exactness

Each block owns the exact 1,049 counters needed by final group entropy:
280 green/length counts, three 256-symbol channel arrays, and one fixed-distance
copy count. Compact uses `u16` for at most 128² token starts; LowLatency uses
`u32` for at most 256² starts. A copy belongs to its starting block. Group
frequencies are rebuilt by merging these counters, never by rescanning tokens.

The same counters derive only the two predeclared proposals. E takes each
channel's exact dominant symbol (smallest symbol on ties) and maps it to the
existing 32-symbol bin. B sums the same counts into eight fixed bins and takes
the greatest mass (smallest bin on ties). Seed ranking, assignment, group cap,
empty fill, and compaction retain the deterministic E37/P14 framework.

`SpatialCostPlan` writes only the cheap prefix structures while planning: the
meta-Huffman flags/block bits, complete nested group map, and all five table
headers per group. Main-token cost is then exact weighted code lengths plus
length/distance extra bits. Payload bit length, rounded payload bytes, padded
RIFF bytes, and writer output are checked before returning. E wins an E/B byte
tie. Refinement starts from the lower-RIFF E/B proposal, uses that proposal's
actual adaptive code lengths, treats absent symbols as infeasible, retains the
original group on equal cost, otherwise takes the lowest group id, and rebuilds
and compacts exactly once. Final ties prefer E, then B, then refined; exact
single wins any complete-RIFF tie, preserving strict fallback. Production
writes only the selected main token stream.

The first smoke incorrectly conflated an absent `(0,0)` table entry with the
legal zero-bit code of a one-symbol table. That run is retained under
`invalidated-runs`; the correction is part of the fixed rate model, not a new
heuristic.

## Phase A: locked 102 images

The manifest hashes are
`9feb09f469753c43864011aa6f00cfc5ee1bd48da5aac8f9b16d105890e14f86`
for all 102 images and
`474587feabe3178268b2eab6f7a166501d8ecc3d637a76bd412f4233dfa7b913`
for the first 41. The Phase A release test binary was
`bd58ddf41960c6bc3d5c5eed1fc6ea4856a0bb684c37cf5d77c328396e4a4ac7`.

All 204 E/B profile rows matched planner payload bits, payload bytes, and RIFF
bytes to the actual writer; all 204 selector decisions matched actual E/B file
sizes, including the fixed tie rule; all 204 public outputs matched the audit
selection. E39's offline oracle was reproduced exactly:

| oracle | Compact | LowLatency |
| --- | ---: | ---: |
| ordered bytes | 617,958,802 | 625,321,072 |
| min(E,B) bytes | 617,342,156 | 625,012,718 |
| delta | -0.099788% | -0.049311% |
| E/B wins | 52 / 50 | 53 / 49 |
| images over +2% | 11 / 102 | 9 / 102 |
| worst | +3.601882% | +7.502532% |

The single fixed refinement materially improved both aggregates:

| Phase A rate | Compact | LowLatency |
| --- | ---: | ---: |
| E bytes | 620,712,252 | 627,958,884 |
| B bytes | 620,360,862 | 627,938,112 |
| refined bytes | 599,398,064 | 617,047,520 |
| final bytes | 599,398,064 | 617,047,520 |
| final vs E37 | **-3.003556%** | **-1.323089%** |
| final worst image | +1.490531% | +7.007527% |
| final images over +2% | 0 / 102 | 3 / 102 (`008`, `066`, `074`) |
| final E/B/refined wins | 0 / 1 / 101 | 7 / 4 / 91 |

Phase A required aggregate bytes, not the later screen tail gate, so both
profiles qualified for the screen. Timings are one locked full-corpus audit
pass and deliberately exclude serialization of the diagnostic E/B outputs:

| attributed work | Compact | LowLatency |
| --- | ---: | ---: |
| counter initialization | 0.000494 s | 0.000299 s |
| exact counter update | 0.618471 s | 0.611267 s |
| E proposal | 0.044935 s | 0.012638 s |
| B proposal | 0.014897 s | 0.005487 s |
| E / B exact costing | 0.051300 / 0.045406 s | 0.029409 / 0.026278 s |
| reassignment | 0.138875 s | 0.035816 s |
| rebuild/compaction | 0.010678 s | 0.003101 s |
| refined exact costing | 0.048561 s | 0.028130 s |
| final selection | 0.000004 s | 0.000007 s |
| exact counter updates | 973,053,692 | 973,053,692 |
| nonzero merge updates per plan | 4,236,492 | 1,456,945 |

Maximum measured accounted counter/plan storage was 993,560/413,904 bytes.
The maximum-dimension counter arrays are 34,373,632/17,186,816 bytes; a
conservative peak including every simultaneously retained research plan/table
stays below 40 MiB.

## Fair 41-image same-binary screen

An independent pre-screen review verified that control and candidate share
prepare/tokenization, exact `SinglePlan`, strict fallback, `SpatialCostPlan`,
nested-map/table writing, and the selected-only packed token writer. Only
spatial construction/planning differs. The final screen test binary is
`65f3fc5753a04efa70b0aacab733d3c191097bf39cea3cb7ef225b0c0ed29404`
(1,573,600 bytes). Inputs were preloaded; the global lock, warmups, three
forward/reverse rounds, full-output checksums, every per-image sample, process
CPU/RSS, MAD, and 3xMAD flags are retained.

| screen metric | Compact | LowLatency |
| --- | ---: | ---: |
| control encode median | 3.341012 s | 3.177514 s |
| candidate encode median | 2.452132 s | 2.240757 s |
| independent encode delta | **-26.605%** | **-29.481%** |
| paired encode delta median | -27.689% | -28.773% |
| per-image encode regressions | 0 / 41 | 0 / 41 |
| aggregate bytes delta | -3.482% | -1.738% |
| worst image bytes | +1.490531% (`002`) | **+4.338207% (`008`)** |
| images over +2% | 0 / 41 | **1 / 41** |
| encode RSS delta | +3,260,416 B / +0.558% | -11,206,656 B / -1.900% |
| Rust decode median delta | -1.700% | **+1.179%** |
| pinned C decode median delta | -0.969% | -0.784% |
| gate | pass | **fail: rate and Rust decode** |

Compact Rust decoder medians were 1.765963/1.735946 s; LowLatency were
1.712918/1.733120 s. Pinned C medians were 2.253341/2.231500 s and
2.224250/2.206819 s. Aggregate decoder gates use independent medians; paired
deltas and all outliers remain in `gate-summary.json` and raw process files.

The same binary's generator produced Default, single, both ordered controls,
and both candidates. The project decoder validated 246/246 screen streams and
pinned libwebp validated 246/246, all complete RGBA exact. A broader archive
check then proved 102/102 Default streams fully byte-identical across creation
base, E37, and P15; the project decoder and pinned libwebp each validated
918/918 Default/Compact/LowLatency archive streams.

## Quality and resources

From the repository root, stable-host debug and release workspace all-targets,
release build, Clippy `-D warnings`, fmt check, rustdoc `-D warnings`, and
doctest all passed. No dependency, public API, unsafe, thread, metadata,
animation, Default, or error-semantics change was found. Only the installed
`aarch64-apple-darwin` host was used.

Relative to E37, the release rlib grew 462,384 -> 574,488 bytes
(+112,104/+24.245%), and the release test binary grew 1,523,552 -> 1,573,600
bytes (+50,048/+3.285%). The test binary includes controls and research-only
instrumentation.

The first validation logs were accidentally rooted under
`webp-rs/experiments`; they all passed but are invalid as final evidence. They
were moved intact to `invalidated-runs/wrong-validation-output-root`, the stray
tree was removed, and all seven commands were rerun from the repository root
to the correct fixed output directory. A pinned-C compile with the wrong
static-archive path is likewise retained as a non-run.

The completed `reproduce.sh` was then invoked from the repository root and
exited zero after validating its 128-file output manifest. Its rebuilt P15 test
binary SHA-256 was
`4fe3c1f20192bb599e66973a787648ce335492f87f4076b150f93ce489895172`;
the reproduction `SHA256SUMS` file hashes to
`70b7f41acd6a1a328afd90ae4525195a50a43b64a44804758cbb8d5250093e62`.
It reproduced the deterministic Phase A totals and image 008's +4.338207%
tail. Its encode deltas were -30.949%/-29.997%, with 0/41 per-image encode
regressions, and all correctness/quality denominators passed again. The
LowLatency Rust decode delta in this independent run was -0.399%, so the
secondary +1% decoder failure did not reproduce; the predeclared +2% rate
failure did, and alone requires the same formal/product rejection.

## Evidence and reproduction

- `phase-a-summary.json` and `gate-summary.json`: machine-readable decisions,
  exact rates, independent/paired medians, per-image rows, resources, and MAD;
- `raw/phase-a-102`: every exact-cost and timing row;
- `raw/screen-41-*`: all final encode/decoder rounds, processes, resources,
  complete-output hashes, and correctness;
- `raw/identity-306-final`: three-archive identity and 918 pinned-C rows;
- `raw/validation-final`: correctly rooted seven-command quality logs;
- `raw/reproduction-final.log`, `raw/reproduction-final-status.txt`, and
  `reproduction-summary.json`: repository-root one-command reproduction;
- `invalidated-runs`: every failed or superseded invocation, preserved and
  named;
- `raw/formal-102x5-not-run.txt`: explicit gate-driven stop;
- `reproduce.sh` and `SHA256SUMS`: one-command archive validation and relative
  evidence hashes.

The narrow positive result is that exact block ownership plus one rate-aware
reassignment can retain large encode gains and improve aggregate rate. It is
not deployable under the declared tail/decode envelope. The product decision
is therefore unambiguously **reject**.
