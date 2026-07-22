# P25 fused entropy-map frequency-sink experiment

This design is frozen before implementation or corpus timing. P25 is an
isolated A/B research tree and will never be merged into `main`.

## Identity

- Task: P25 independent VP8L fused entropy-map sufficient-statistics +
  rank-sum recovery experiment
- Root task: `019f8321-035e-7211-8f53-987e18891c8c`
- Branch: `codex/vp8l-entropy-map-frequency-sink`
- Worktree: `/Users/lance/.codex/worktrees/8312/webp`
- Base, creation `HEAD`, local `main`, and merge-base:
  `acfe6caf9fb62468dc384790b3e2eecfe837f173`
- Creation was clean and detached; both ancestry directions succeeded before
  attaching this branch.

Read-only controls are P20 at `/Users/lance/.codex/worktrees/5020/webp`
`cebc0981c23d7b4e719b1491ee83907560e0bd63` (only source commit
`67bd04274a60fe81fcd13cc2702d75e3fd0553a4`), P24 at
`/Users/lance/.codex/worktrees/a5b2/webp`
`e5beb22e46bd9239511ccc6d8c048ca684020c3e` (mechanism
`7fbe02d4fb7da3100d52d4816e3749875042549d`, audit correction
`711414241bd2402a150ef365969a7496c6d2886e`, runner
`17d623f4e2d304edd2f398e894455641efef7649`), and P18 at
`/Users/lance/.codex/worktrees/7d78/webp`
`c04bed7bf044dc610081ff1de0e43a2a579258bb`.

## Frozen hypothesis and invariant

A is P20's retained-full-plan search and release route. O uses P24's
allocation-free rank-sum metric while retaining the generic nested entropy-map
collector: synthesize `[0, group, 0, 0]`, collect residual/token vectors, then
rank-sum their exact frequencies. B is O plus a narrow private generated-pixel
to exact-frequency sink owned by entropy-image costing.

B scans assignments directly. For every maximal equal-group run it emits one
logical literal; then, at each remaining position, emits a distance-one copy
only if the remaining run is at least three, capped at 4096, otherwise emits a
literal. A literal increments green[group] and red/blue/alpha[0]. A copy
increments the exact VP8L length-prefix and `vp8l_prefix(121, 40)` distance
prefix counters, including exact extras and checked error behavior. The five
histograms go directly to P24 rank-sum.

Candidate B materializes none of the synthetic RGBA, residual, token,
adaptive/canonical-table, code/length, prefix, or cross-group table vectors.
`PlanParts` may still allocate. The winning plan alone uses the existing real
collector, table builder, and writer exactly once. Proposal/refinement/growth,
tie and stop rules, strict fallback, profiles, wire, limits, errors, and public
API stay unchanged. A/O/B dispatch and census are `cfg(test)` outside timing;
release remains A for this experiment.

## Gates

Before B, A must reproduce Compact/LowLatency totals 599,398,064 / 601,400,998
and P18 stream size/hash 204/204. Unit tests compare B with O over runs
0..=8193, all groups at critical lengths, alternating/distinct runs,
map-width boundaries, randomized sequences, legal maxima, and overflow/error
edges. They prove frequencies, copy segmentation/prefixes/extras, rank-sum
bits/bytes/RIFF, and error categories equal, while the B-only materialization
and tokenization census is zero.

The non-timed audit requires A/O/B/public/P18 204/204 identity; Compact
E/B/R metric and planner/writer 306/306 and zero growth; LowLatency E/B/R and
attempted Split metric 642/642, planner/writer 408/408, growth 336/336; equal
selectors/fallbacks/errors. O generic nested-map tokenizations must be
306/642; B direct-map evaluations 306/642 and generic tokenizations plus
synthetic RGBA/residual/token allocations zero.

The sole recovery is one release binary, preload, one unscored warmup, then
three interleaved F/R/F A/B rounds. LowLatency needs at least 5.0% improvement
and 0/41 median regressions; Compact may regress less than 0.5% and reports
every regression; A/B streams must match 82/82 with empty stderr. Any failure
stops, retains release A, and commits a negative report/reproducer. A pass
still only recommends a fresh latest-main product migration with its own
at-least-50% product screen and formal gates.
