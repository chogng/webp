# P16: capacity-growing exact-cost split/refine clustering

## Current decision

**Reject at the 41-image screen; formal 102x5 was not run.** Phase A passed,
but Compact encode regressed 79.086400% with 40/41 per-image median
regressions. LowLatency passed every screen gate, but the user required both
profiles to pass before formal measurement.

## Provenance and fixed rule

The branch is `codex/vp8l-capacity-growing-clustering`, its creation base is
`ec7fbaf69f423bfd7251a121d2e629cfa776cb79`, and the worktree is
`/Users/lance/.codex/worktrees/5d9b/webp`. Before modification, worktree HEAD,
local `main`, and `merge-base HEAD main` independently equalled that SHA.
Post-creation registration `cb89e317` is recorded only; this branch was not
rebased or merged. The immutable rule checkpoint is `7641d33a`; implementation
checkpoint is `31595aa3`.

The full support-safe distance, regret, seed, partition, reassignment, tie,
acceptance, and stop rules are in [DESIGN.md](DESIGN.md). The defining distance
is exact `C(a+b)-C(a)-C(b)`, where `C` includes real five-table headers and
payload. The combined model contains the union of both supports, so no
singleton is evaluated under another singleton's absent-symbol codebook and
no pseudocount smoothing is used.

Research checkpoints are `7641d33a` (fixed rule), `31595aa3` (implementation),
`0ba25f17` (initial passing Phase A), `cea589b5` (scalar traces), `58327b09`
(shared E37 preparation), and `e1b6c851` (final rejected screen evidence).

## Phase A: locked 102 images

The regenerated 102-row manifest SHA-256 is
`9feb09f469753c43864011aa6f00cfc5ee1bd48da5aac8f9b16d105890e14f86`;
the first-41 hash is
`474587feabe3178268b2eab6f7a166501d8ecc3d637a76bd412f4233dfa7b913`.
The final release Phase A and screen test binary is `298abf0e…8ee`
(2,220,144 bytes). Earlier `034ae904…f01c`, `fac7d627…bc7`, and
`1828e721…d84e` Phase A runs are retained as superseded fairness checkpoints;
all 4,518 keyed non-timing fields match across all four runs.

All 816 E/B/refined/split plan writes matched predicted payload bits, payload
bytes, and complete RIFF bytes. All 204 single plans also matched. E/B selector,
final selector, and public output each matched actual bytes for 204/204 rows.

| rate metric | Compact | LowLatency |
| --- | ---: | ---: |
| E37 control bytes | 617,958,802 | 625,321,072 |
| E40 bytes | 599,398,064 | 617,047,520 |
| P16 final bytes | **547,448,078** | **601,400,998** |
| P16 vs E37 | **-11.410263%** | **-3.825247%** |
| P16 vs E40 | **-8.667026%** | **-2.535708%** |
| worst image vs E37 | -1.468297% (`082`) | +1.483745% (`100`) |
| images over +2% | 0 / 102 | 0 / 102 |
| E/B/R/Split/Single wins | 0 / 0 / 0 / 102 / 0 | 1 / 2 / 20 / 79 / 0 |
| growth attempts / accepted | 3,978 / 3,967 | 336 / 336 |

Every accepted step strictly reduced complete RIFF bytes. Eleven Compact
images stopped at their first non-improving candidate; all other paths stopped
at the profile cap. No rate-degrading candidate was accepted.

### E40 LowLatency tails

| image | refined groups | final groups | E40-refined bytes | P16 bytes | E37 bytes | P16 vs E37 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 008 | 11 | 16 | 4,582,148 | 4,302,942 | 4,391,630 | -2.019478% |
| 066 | 7 | 16 | 3,605,670 | 3,355,168 | 3,463,560 | -3.129497% |
| 074 | 5 | 16 | 5,440,680 | 4,535,584 | 5,084,390 | -10.793940% |

[TAILS.md](TAILS.md) lists every iteration's group count, source, seeds,
regret, support-safe merge penalty, partition sizes, exact RIFF delta, and
acceptance. These three images all improve when capacity grows, which supports
the P16 mechanism on the declared tails; it does not by itself prove a general
causal law relating group count and rate.

## Attribution and resources

The instrumented final-binary Phase A pass was for attribution, not a
performance claim. Compact totals were 0.537 s counter updates, 0.441 s self
costs, 2.083 s regret, 0.586 s seed selection, 1.006 s partition, 0.409 s split
rebuild, 7.108 s global reassignment, 0.419 s final rebuild, and 3.285 s exact
candidate costing. LowLatency totals were respectively 0.541, 0.134, 0.053,
0.036, 0.057, 0.009, 0.100, 0.009, and 0.091 s. Selection comparisons were
below 0.001 s per profile. Both profiles performed 973,053,692 exact counter
updates.

Maximum observed block counts were 192/48. Maximum observed counter plus
self-cost storage was 405,888/202,176 bytes, and maximum retained E/B/R/split
plan storage was 907,624/283,328 bytes. At 16384², exact block counters plus
self-cost caches are 34,635,776/17,252,352 bytes; a conservative bound with
four retained maximum-group plans remains below 40 MiB. This is below the
screen's +64 MiB envelope, which must still be measured at process level.

Against the same-source no-feature control, the release test binary grew
2,118,448 -> 2,220,144 bytes (+101,696/+4.800%), and release rlib grew
2,102,136 -> 2,333,376 bytes (+231,240/+11.000%).

## Fair 41-image same-binary screen

The final binary directly reused E37's actual `Prepared`, tokenization, fast
prefix, SinglePlan, strict fallback, and packed writer for both control and
candidate. Only clustering/planning differed. Inputs were preloaded; one
warmup preceded three measured forward/reverse/forward rounds. All stream
hashes were stable. Medians below are aggregate in-process time; JSON retains
all samples, process CPU/wall/RSS, MAD, 3xMAD flags, and per-image medians.

| gate metric | Compact | LowLatency |
| --- | ---: | ---: |
| E37 encode median | 4.939605 s | 4.843859 s |
| P16 encode median | 8.846161 s | 2.560232 s |
| encode delta | **+79.086400% (fail)** | **-47.144794% (pass)** |
| per-image encode regressions | **40 / 41 (fail)** | **0 / 41 (pass)** |
| aggregate rate delta vs E37 | -13.092110% | -5.093032% |
| worst per-image rate delta | -3.169255% (`011`) | -1.128722% (`031`) |
| Rust decode delta | +0.482386% | -1.444946% |
| pinned C decode delta | +0.682189% | -0.830949% |
| process RSS delta | -321,880,064 B / -35.476% | -323,420,160 B / -35.638% |
| screen gate | **fail** | pass |

Both rate gates, both decoder <=+1% gates, and both RSS gates passed. Project
Rust decoded all 246 control/candidate/auxiliary streams to exact RGBA; pinned
libwebp `733c91e` independently matched 246/246. Compact's accepted 3,978-way
capacity-growth search is rate-effective but computationally unsuitable in
this fixed architecture. The data supports that narrower conclusion; it does
not establish group count as a general rate cause.

Because both profiles did not pass, `raw/formal-102x5-not-run.txt` is the
terminal formal result. Default byte identity, 102-image formal dual-decoder
validation, and the formal all-workspace gate suite were intentionally not run.
The proportionate research checks passed: feature lib tests 278/278, default
lib tests 273/273, feature/default clippy with `-D warnings`, stable fmt check,
Python summary syntax, shell reproduction syntax, and the full reproduction
itself. All production P16 modules are below 500 lines; new tests are sibling
files.

## Evidence

- `phase-a-summary.json`: machine-readable rate, exactness, timing, memory,
  tail, and gate summary;
- `raw/phase-a-102-final-screen-binary/phase-a.tsv`: final-binary 204
  image/profile rows and every split candidate; stderr is empty;
- the other three `raw/phase-a-102*` directories: superseded fairness runs with
  identical non-timing fields;
- `raw/corpus-manifest-102.tsv`, `MANIFEST.md`: locked inputs and hashes;
- `raw/binary-artifacts-phase-a.tsv`: exact binary/rlib sizes and hashes;
- `screen-summary.json`: complete screen samples, medians, MAD/outliers,
  per-image timing/rate, RSS, correctness, and gate decisions;
- `raw/screen-41-*-final`: final encode/decode/correctness evidence;
- `raw/validation-final`: proportionate feature/default test, clippy, fmt, and
  stable toolchain logs;
- `raw/reproduction-final-status.txt`: successful isolated one-click replay;
- `invalidated-runs`: all invalid or superseded harness attempts and reasons.
