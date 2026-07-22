# P21 sparse exact-histogram merge recovery

This design is frozen before any P21 corpus performance output. P21 isolates
one implementation rule in the P20 product architecture: whether merging a
per-block exact histogram invokes `checked_add` for source counters equal to
zero. No other algorithm, route, or threshold is an experimental variable.

## Provenance and fixed scope

- Task: `P21 sparse exact-histogram merge recovery`
- Root task: `019f8321-035e-7211-8f53-987e18891c8c`
- Branch: `codex/vp8l-sparse-histogram-merge`
- Worktree: `/Users/lance/.codex/worktrees/1841/webp`
- Base, creation `HEAD`, local `main`, and merge-base:
  `8485fc0593bf6e29715350ea72b15a9dabf4c80b`
- Read-only P20 product source: `codex/vp8l-profile-hybrid-product` at
  `cebc0981c23d7b4e719b1491ee83907560e0bd63`, worktree
  `/Users/lance/.codex/worktrees/5020/webp`; only production commit
  `67bd04274a60fe81fcd13cc2702d75e3fd0553a4` may be cherry-picked after this
  design is committed.
- Read-only P18 oracle: `codex/vp8l-profile-hybrid-clustering` at
  `c04bed7bf044dc610081ff1de0e43a2a579258bb`, worktree
  `/Users/lance/.codex/worktrees/7d78/webp`.
- Historical P20 product binary SHA-256:
  `9aa8fa08fb2288335a98ff4a3d5f64a5a7922372f7c4c1b1212ed98b9b1a29f8`.
- Historical P18 research binary SHA-256:
  `05b8421c86f3286667c5cffef35ffc2bff77f68d944063a954793e2b870e64c9`.

P21 will rebuild its own private A/B oracle, counters, runners, summarizers,
and evidence directory. It will not cherry-pick or copy P20 design, audit,
evidence, runner, or report commits, and it will not merge or rebase P18, P20,
or `main` after branch creation.

## Exact variants and proof obligation

P20's `BlockFrequencies::merge_into` owns five fixed source arrays totaling
1,049 counters: green/length, red, blue, alpha, and copy/distance. For each
source/destination slot, variant A always evaluates
`destination.checked_add(source)` and reports the existing overflow error on
failure. Variant B first evaluates `source != 0`; it skips the addition when
false and otherwise performs the identical `checked_add` and error mapping.

The proof obligation is that A and B return identical destination frequencies
and identical errors for every representable source/destination pair. For a
zero source, `x.checked_add(0) == Some(x)` for every counter value, so B's skip
is observationally identical. For a nonzero source, B executes the exact A
operation. Therefore the first overflowing nonzero slot, traversal order,
partial destination state at error, and returned `EncodeError` are identical.
Tests must cover empty, sparse, dense, maximum-count and overflow-adjacent
histograms, literal and copy tokens, the `u16` and `u32` block counter paths,
and deterministic plans.

The algorithm rule is fixed for all images, blocks, stages, and both profiles.
There is no image/size classifier or corpus tuning.

## Census definitions and lifetime

The test-only oracle will expose A and B only inside one final test binary.
Production callers cannot select A or B. Census state is owned by test-only
merge orchestration and is passed for one encode, aggregated by profile and
planning stage, emitted after the corpus run, and then dropped.

- `slot_visits`: number of source slots inspected. A and B both inspect all
  1,049 slots per block merge, so this is independent of the add decision.
- `nonzero_adds`: visited slots whose source counter is nonzero and for which
  both variants execute the same checked addition.
- `skipped_zero`: visited slots whose source counter is zero. A executes a
  checked addition of zero; B skips it.
- `zero_elision_ratio = skipped_zero / slot_visits`; it is reported only when
  `slot_visits > 0`.

Stage ownership follows P20's private modules: exact block-frequency storage
owns source histograms; spatial refinement owns assignment rebuilds and calls
the merge helper; profile orchestration owns the test-only A/B choice and
per-profile census lifetime. Counters and the A/B switch are `cfg(test)` only
and cannot remain in release/product routing. Census is never enabled in a
timed measured build.

## Fixed production behavior and exclusions

Apart from the merge rule, P20 production behavior is immutable: clustering,
proposal construction, exact-cost refinement, capacity growth, group caps,
ties and stop rules, counter widths, selected-only writer, profile routing,
image selection, thresholds, parallelism, dependencies, public API, `Default`,
metadata, animation, input limits, allocation failures, and error mapping do
not change. The implementation remains safe Rust and single-threaded and adds
no feature, public hook, dependency, thread, or unsafe block.

Before any A/B measurement, variant A must reproduce P20/P18 product bytes.
Release routing may use B only after every recovery gate passes. If recovery
passes, the dense A route and all counters/switches are removed from the
release candidate; the product gates use one locked final B-only binary.

## Locked corpora and external evidence

The CLIC 1.0.0 validation corpus contains exactly 102 source PNGs. Its locked
manifest SHA-256 is
`9feb09f469753c43864011aa6f00cfc5ee1bd48da5aac8f9b16d105890e14f86`.
The locked recovery/product screen is the first 41 rows and its manifest
SHA-256 is
`474587feabe3178268b2eab6f7a166501d8ecc3d637a76bd412f4233dfa7b913`.
Expected Compact and LowLatency aggregate output sizes are respectively
599,398,064 and 601,400,998 bytes.

Durable design, report, concise JSON summaries, provenance, runner and
summarizer sources, and named invalidation notes live in
`experiments/vp8l-sparse-histogram-merge`. Raw TSV, stderr, logs, run
directories, binaries, and `SHA256SUMS` live only in an explicit external
output directory and are not added to Git. All valid and invalidated results
are retained. A cross-target rebuild may change the binary hash, but one SHA
must remain unchanged within each replay phase.

## Phase R: mechanism, identity, and recovery gate

1. Unit/property tests must prove A/B frequency and error identity over the
   cases in the proof obligation and deterministic plan construction.
2. One final test binary must report per-profile/per-stage `slot_visits`,
   `nonzero_adds`, `skipped_zero`, and `zero_elision_ratio` for all 102 images,
   without counters in timed builds or the release candidate.
3. A, B, the public candidate, and the P18 candidate must be byte-identical on
   102 images for both profiles: 204/204 for each pair, with exact aggregates
   above. Planner/writer, E/B selector, final selector, public selected stream,
   and strict fallback exactness must pass; Compact growth is 0/0 and
   LowLatency growth is 336/336.
4. The locked 41-image same-final-test-binary recovery screen preloads inputs,
   performs one warmup, then retains three forward/reverse/forward interleaved
   rounds of full end-to-end A and B. All 82 profile/image output pairs must be
   byte-identical and every stderr file empty. LowLatency B versus A must show
   at least 3.0% independent aggregate encode improvement and 0/41 per-image
   median regressions. Compact aggregate encode time may regress by at most
   1.0%, and every Compact per-image regression is reported.

No outlier may be removed and no valid screen may be repeated to seek a pass.
Any recovery failure stops the experiment, prohibits product Phase A, product
screen, and formal, and requires a committed negative report/reproducer with a
rejection recommendation.

## Product gates, only after Phase R passes

1. Remove dense production routing and all A/B/census machinery from release.
   Lock the final B-only binary and rerun the 102-image Phase A from scratch.
   It must reproduce P18 bytes and the fixed aggregates, have 0/102 images over
   control by more than 2%, pass every exactness denominator, preserve Compact
   no-growth state, and report LowLatency growth 336/336.
2. Run P20's fair 41-image screen from warmup on that unchanged binary against
   latest-main/E37 same-binary controls. Independently for both profiles it
   must improve aggregate encode time by at least 50%, have 0/41 per-image
   encode regressions, no aggregate byte growth, 0/41 images over control by
   more than 2%, Rust and pinned-C decode regressions below 1%, RSS increase
   below both 64 MiB and 5%, and project/pinned-C exactness for all six layouts.
   Any failure stops and prohibits formal.
3. Only after the product screen passes, run locked 102-image formal timing on
   the unchanged binary: one warmup and five retained F/R/F/R/F rounds. Each
   profile must improve its independent median by at least 50%, have 0/102
   per-image median regressions, and have absolute candidate median at most
   7.1 seconds for Compact and 6.9 seconds for LowLatency.
4. Only after formal passes, require project and pinned-C exactness for all
   required layouts on 102 images, product/no-product `Default` byte identity
   102/102, stable default and relevant workspace all-target tests, Clippy all
   targets with warnings denied, formatting, rustdoc with warnings denied,
   doctests, API/dependency/unsafe/thread/module/resource audits, rlib/binary
   size audits, and a repository-root isolated-target replay of mechanism,
   Phase A, screen, formal, correctness, identity, and quality.

Only the installed stable host target is permitted. Passing every gate
recommends promotion without integrating `main`; any hard failure recommends
rejection and preserves its evidence on this branch.
