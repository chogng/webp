# P17: multi-resolution exact-cost spatial portfolio

## Decision

**Reject P17 at Phase A and do not run the 41-image screen or 102x5
formal benchmark.** Compact reproduced P15 exactly, and the LowLatency
portfolio improved aggregate bytes substantially, but LowLatency image 074
was 4.992654% larger than its E37 control. The declared per-image limit is
+2%, so this single deterministic rate failure is sufficient to stop.

## Provenance

The independent worktree is
`/Users/lance/.codex/worktrees/dfbc/webp` on
`codex/vp8l-multires-spatial-portfolio`. Before any modification, worktree
HEAD, local `/Users/lance/Desktop/webp` main, and `merge-base HEAD main`
were all exactly
`ec7fbaf69f423bfd7251a121d2e629cfa776cb79`. The later main registration
`cb89e317` is recorded only as post-creation provenance; this branch was not
rebased or merged. P15 `76762d10` was read-only reference material and was
not cherry-picked or merged.

Implementation checkpoints are `41c24db5` and `bdb709ea`; the complete
Phase A evidence checkpoint is `c151f06b`. The locked
102-image manifest has SHA-256
`9feb09f469753c43864011aa6f00cfc5ee1bd48da5aac8f9b16d105890e14f86`.
The final Phase A test binary is 2,217,120 bytes with SHA-256
`42ec743c6791319a8186882380b9865eccc8acb9ff00993f999842d42cb75629`.

## Architecture

The implementation rebuilds P15 exact block-frequency ownership, fixed E/B
proposals, one code-length reassignment, exact `SpatialCostPlan`, strict
single fallback, and selected-only packed writing. Compact constructs only
the 128-block resolution. LowLatency prepares tokens once, constructs the
128-block and 256-block exact winners sequentially, releases each large
counter before constructing the next resolution, and chooses the smaller
complete RIFF stream. A RIFF tie selects 256; single wins every complete-RIFF
tie.

Both resolutions rescan the shared prepared tokens for their own exact
counters, and all counter/proposal/reassignment/cost work remains inside the
public encode path. The output is ordinary VP8L. There is no classifier,
image identifier, decode timing, threshold, third block size, second
refinement, dependency, unsafe code, thread, public API, Default, metadata,
animation, or error-semantics change.

## Locked Phase A

All 204 resolution rows matched planned and written payload bits, payload
bytes, and complete RIFF bytes. The audit performed 816 exact planned-stream
checks across E, B, refined, and the reconstructed winner. All 102 resolution
selectors matched the actual smaller file, all 102 Compact public outputs and
all 102 LowLatency public outputs matched the audit selection.

| rate metric | Compact | LowLatency portfolio |
| --- | ---: | ---: |
| E37 control bytes | 617,958,802 | 625,321,072 |
| P17 bytes | **599,398,064** | **599,169,200** |
| vs E37 | -3.003556% | -4.182151% |
| P15/E40 bytes | 599,398,064 | 617,047,520 |
| vs P15/E40 | 0 B | -17,878,320 B / -2.897398% |
| worst per-image vs E37 | +1.490531% (002) | **+4.992654% (074)** |
| images over +2% | 0 / 102 | **1 / 102** |
| 128 / 256 / single wins | 102 / 0 / 0 | 99 / 3 / 0 |

Compact therefore reproduces P15 exactly. Both aggregate ceilings pass, but
LowLatency fails the required 0/102 tail gate.

The three P15 LowLatency tails show why the portfolio cannot repair the fixed
gate:

| image | 128 exact bytes | 256 exact bytes | selected | Low vs E37 |
| --- | ---: | ---: | --- | ---: |
| 008 | 4,353,944 | 4,582,148 | 128 | -0.858132% |
| 066 | 3,431,440 | 3,605,670 | 128 | -0.927370% |
| 074 | 5,338,236 | 5,440,680 | 128 | **+4.992654%** |

For 074, both exact resolutions lose to the 5,084,390-byte E37 LowLatency
control. Exact portfolio selection picks the less-bad 128 stream but cannot
satisfy the +2% bound.

The only 256 selections were 005, 040, and 068, saving 79,072, 97,356, and
52,436 bytes over 128 respectively. LowLatency selected 128 on 99/102 images,
and all 99 outputs were byte-identical to Compact. It is not a literal alias,
but it is a 97.1%-Compact profile that gives up the intended 256-block latency
semantics on nearly the whole corpus while paying for two resolution plans.
That is weak product differentiation, and no encode/decode performance claim
is made because the screen was prohibited.

## Attribution and resources

One locked audit pass attributed 1.580540 s to shared preparation. Resolution
work was:

| attributed work | 128 | 256 |
| --- | ---: | ---: |
| counter initialization | 0.000503 s | 0.000245 s |
| exact counter update | 0.609374 s | 0.603181 s |
| E proposal | 0.045456 s | 0.012782 s |
| B proposal | 0.015458 s | 0.005518 s |
| E exact cost | 0.053748 s | 0.030614 s |
| B exact cost | 0.047778 s | 0.028199 s |
| reassignment | 0.135800 s | 0.035904 s |
| rebuild/compaction | 0.010762 s | 0.003230 s |
| refined exact cost | 0.051255 s | 0.029732 s |
| candidate selection | 0.000002 s | 0.000002 s |

Final resolution and single selection were 0.000007 s and 0.000008 s. Each
resolution performed 973,053,692 counter updates. Measured maximum accounted
live storage was 993,560 bytes on this corpus. At 16384x16384, the 128 and 256
counter arrays model to 34,373,632 and 17,186,816 bytes; sequential ownership
keeps the conservative peak below 40 MiB.

Relative to creation base, the release rlib grew 2,164,896 -> 2,341,872 bytes
(+176,976 / +8.175%). The release test binary grew 2,117,168 -> 2,217,120 bytes
(+99,952 / +4.721%); it includes audit controls and instrumentation.

## Invalidated and gated-off work

The first complete Phase A run had valid rate/exactness results, but its
selection timer included diagnostic output cloning and its selected-128 field
encoded an implication rather than the requested count. It is preserved
unchanged under
`invalidated-runs/superseded-selection-attribution`. The corrected binary
was rebuilt and all 102 images were rerun; only this second run is cited
above. A subsequent rebuild from committed `bdb709ea` reproduced the exact
binary SHA `42ec743c…5629`.

The first binary inventory ran before a candidate rlib existed and is retained
under `invalidated-runs/missing-candidate-rlib`. The preliminary 008 smoke is
retained separately. The 41-image same-binary screen, decoder gates, 102x5
formal benchmark, Default archive identity, full pinned-C archive validation,
and final stable product gates were not run because Phase A failed.

Two reproducer preflights also stopped before Phase A: one used an incorrectly
expanded full commit SHA, and one incorrectly required test-binary byte
identity across different scratch build paths. Both partial runs are retained
under `invalidated-runs`. The portable reproducer records its isolated binary
SHA while validating the deterministic codec results instead.

The final one-command reproduction exited zero. Its isolated test binary SHA
was `ed0c685c…f5ffb8`; it reproduced both deterministic aggregates, the 074
failure, all exactness denominators, and the 99/3 resolution split. Its output
`SHA256SUMS` hashes to `3389a655…41f9e`.

## Evidence and reproduction

- `phase-a-summary.json`: machine-readable decision, exactness, rates,
  selector semantics, timing, memory, and artifact deltas;
- `raw/phase-a-102/phase-a.tsv`: final 102-image row-level exact evidence;
- `raw/final-binary-rebuild.tsv`: committed-source binary SHA proof;
- `invalidated-runs`: superseded Phase A and malformed artifact inventory;
- `raw/screen-41-not-run.txt` and `raw/formal-102x5-not-run.txt`: hard-stop
  records;
- `reproduce.sh`, `summarize.py`, and `SHA256SUMS`: one-command
  reproduction and relative evidence hashes.

From the repository root:

```bash
experiments/vp8l-multires-spatial-portfolio/reproduce.sh
```

A passing research result would still require a fresh product migration
worktree from the then-latest local main. P17 is negative, so no migration is
recommended.
