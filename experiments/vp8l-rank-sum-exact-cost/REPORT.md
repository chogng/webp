# VP8L allocation-free rank-sum exact-cost report

## Decision

Reject promotion. P24 proved that allocation-free rank-sum is an exact,
general replacement for adaptive/canonical table construction during candidate
costing, but the only valid recovery screen did not reach the locked
LowLatency speed threshold. LowLatency improved 3.5162819256%, below 5.0%,
with 0/41 per-image median regressions. Compact improved 1.9943819161% and
passed its aggregate gate, with four recorded per-image regressions.

Release therefore remains retained-plan A. Product Phase A, the product
screen, formal 102x5 timing, final product correctness/identity/quality gates,
and isolated product replay were prohibited and not run. There was no valid
rerun to seek a pass.

## Provenance

- Task: `P24 independent VP8L allocation-free rank-sum exact-cost experiment`
- Root task: `019f8321-035e-7211-8f53-987e18891c8c`
- Branch: `codex/vp8l-rank-sum-exact-cost`
- Worktree: `/Users/lance/.codex/worktrees/a5b2/webp`
- Base, creation HEAD, local main, and merge-base:
  `230ce0bd1c201d2687261d97a525cec8f91aa215`
- Measurement HEAD: `17d623f4e2d304edd2f398e894455641efef7649`
- Locked P24 binary SHA-256:
  `e8ccb712bd30c4486edf6502f172d1804f7682ffea12fcc903c2736f601df22b`
- Rebuilt P18 binary SHA-256:
  `2e5f7b11b959de3cb25a251649ee7ffa87528b0346d92fdfbb03547da5f5e570`
- Pre-B A binary SHA-256:
  `dfe428f80c90cbf3863fbc9e543d7f6df1dd3af7a9202b475dadb507da20f617`
- Corpus manifest SHA-256:
  `9feb09f469753c43864011aa6f00cfc5ee1bd48da5aac8f9b16d105890e14f86`
- Screen manifest SHA-256:
  `474587feabe3178268b2eab6f7a166501d8ecc3d637a76bd412f4233dfa7b913`
- External Phase R `SHA256SUMS` SHA-256:
  `8d3ca8831a052e7c9536fc7fdd8a251cd053b2f218aca917ca9dd764d1416bab`
- External output:
  `/private/tmp/vp8l-rank-sum-exact-cost-p24-phase-r-17d623f4`

The coherent commit chain before this report is:

1. `acaf6ebe` design/provenance freeze;
2. `7edb006a` selective P20 production transplant as retained A;
3. `20eafd31` locked pre-B baseline runner;
4. `fe77511e` passed A/P18 baseline evidence;
5. `61ddfad5` narrow allocation-scope clarification;
6. `7fbe02d4` test-only O and rank-sum B mechanism;
7. `71141424` multi-maximum-counter and conservative storage audit correction;
8. `17d623f4` locked mechanism/census and one-shot recovery runner.

P23 code was used only to reconstruct the test-only O mechanism. No P23
evidence or measurement was cherry-picked or claimed as P24 evidence. P18 was
locked clean at `c04bed7bf044dc610081ff1de0e43a2a579258bb` before its binary was
rebuilt.

## Formula and equivalence proof

For `k=0` or `k=1` nonzero symbols, canonical table construction consumes zero
data-symbol bits. For `k>=2`, let `L=floor(log2(k))`, `base=2^L`, and
`m=2*(k-base)`. Kraft equality gives exactly `m` codes of width `L+1` and the
remaining codes width `L`. The existing deterministic builder gives those
long codes to the `m` least-frequent nonzero symbols, so exact data cost is
`L*sum(f) + sum(m least nonzero f)`.

If equal frequency `t` straddles the selection boundary, every possible
selection takes the same number `q` from that tied class and contributes
`q*t`. Symbol tie order can change canonical code values, but cannot change
the frequency-weighted width sum. P24 therefore uses a checked fixed
`[u32; 296]` stack scratch and `select_nth_unstable` without reproducing symbol
tie order, heap fallback, or unsafe code.

Tests covered every alphabet length 1..=296; empty, one-symbol positions,
sparse, dense, all-tied, skewed, deterministic random, and multi-symbol
`u32::MAX` shapes; every power-of-two boundary; small/large long counts; ties
across the boundary; copy prefixes/extras; inconsistent copy failures; and
overflow-adjacent complete metrics. B rank-sum, O real-table weighted widths,
the real table writer, and complete real plans matched throughout.

## Pre-B control

Before B existed, retained A reproduced Compact 599,398,064 bytes and
LowLatency 601,400,998 bytes. A and P18 stream size+hash identity was 204/204,
with empty stderr. Durable details are in `a-baseline-summary.json`.

## Locked 102-image mechanism and census: pass

A/O/B/public/P18 identity was 204/204. Compact candidate metrics and real-plan
comparisons were 306/306, planner/writer was 306/306, and growth was 0/0.
LowLatency candidate metrics covered E/B/R and every attempted Split at
642/642, planner/writer was 408/408, and growth was 336/336. Final selectors,
strict fallback, final materialization, and bytes all matched. Compact selected
B once and R 101 times; LowLatency selected E once, B twice, R 20 times, and
Split 79 times.

| Census | Compact | LowLatency |
| --- | ---: | ---: |
| O adaptive table builds | 35,115 | 43,905 |
| B adaptive table builds | 0 | 0 |
| B canonical table builds | 0 | 0 |
| B rank-sum/table-cost heap allocations | 0 | 0 |
| B histogram evaluations | 35,115 | 43,905 |
| B selection operations | 19,836 | 24,038 |
| B scanned counters | 7,641,024 | 9,553,728 |
| B selected counters | 1,575,724 | 1,862,522 |
| Nested-map tokenizations per O/B variant | 306 / 306 | 642 / 642 |
| Final materializations | 102 | 102 |
| Scratch high-water counters | 271 / 296 | 271 / 296 |
| Maximum A candidate heap estimate | 672,740 B | 220,044 B |
| Maximum O conservative heap estimate | 265,784 B | 116,592 B |
| Maximum B conservative live-storage estimate | 221,176 B | 71,984 B |

The zero-allocation claim is deliberately narrow: it covers only B rank-sum
and table-cost materialization. Candidate `PlanParts` and unchanged nested-map
RGBA/token/residual allocations remain, and are excluded from that zero field.
The O/B live-storage figures are conservative requested-element estimates,
not directly instrumented process heap, and exclude allocator overhead.

## Only valid recovery screen: fail

The runner built one final release test binary, preloaded each invocation's
inputs before timing, performed exactly one unscored warmup round, and retained
the three interleaved F/R/F rounds. All samples remain in the external output.

| Gate | Compact | LowLatency |
| --- | ---: | ---: |
| A aggregate samples | 2.529713375 / 2.558378292 / 2.560324333 s | 2.620144458 / 2.592931417 / 2.642414166 s |
| B aggregate samples | 2.465350875 / 2.507354458 / 2.512765333 s | 2.528012792 / 2.542315500 / 2.517305083 s |
| Independent improvement | 1.9943819161% pass | 3.5162819256% **fail** |
| Per-image median regressions | 4/41 (reported below) | 0/41 pass |
| A/B stream identity | 41/41 | 41/41 |

Compact regressions were `clic-validation-005` +0.2721257573%, `007`
+0.3172519211%, `022` +1.2876196633%, and `023` +0.9520534057%. Compact's
frozen gate required only aggregate regression below 0.5%, so its 1.994%
improvement passed. LowLatency failed only the required improvement of at
least 5.0%. A/B output identity was 82/82 and all recovery stderr files were
empty.

## Verification and stopping point

Before the locked audit, the stable release focused VP8L image-writer suite
passed 43 tests with one intentionally ignored reproducer. The stable debug
library suite passed 305 tests with four ignored, Clippy passed with warnings
denied, and formatting passed after applying rustfmt to one newly added
assertion. The locked binary reran the mechanism suite successfully before
corpus audit.

Because recovery failed, no production routing changed: release remains A and
the test-only O/B dispatch/census does not affect release. No product Phase A,
product screen, formal timing, final all-layout correctness, Default identity,
full workspace/rustdoc/doctest/resource audit, or isolated product replay was
run. The negative result is exact and general but insufficient alone to
justify promotion.
