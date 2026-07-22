# P22 metric-only spatial-plan search and final materialization

This design is frozen before the authorized P20 production transplant, before
P22 implementation, and before any P22 corpus timing. P22 isolates one
ownership/lifetime rule in the P20 product planner. No cost, clustering,
proposal, reassignment, growth, selection, fallback, or wire rule is an
experimental variable.

## Provenance and fixed scope

- Task: `P22 independent VP8L metric-only search / final-plan materialization
  recovery experiment`
- Root task: `019f8321-035e-7211-8f53-987e18891c8c`
- Branch: `codex/vp8l-metric-only-plan-search`
- Worktree: `/Users/lance/.codex/worktrees/c5fc/webp`
- Base, creation `HEAD`, local `main`, and merge-base:
  `4280a59a1a7a22d1e312b9de131b46873688c008`
- Read-only P20 product source: branch
  `codex/vp8l-profile-hybrid-product` at
  `cebc0981c23d7b4e719b1491ee83907560e0bd63`, worktree
  `/Users/lance/.codex/worktrees/5020/webp`. After this design commit, only
  production commit `67bd04274a60fe81fcd13cc2702d75e3fd0553a4`
  may be cherry-picked as the exact A control. The resulting P22 cherry-pick
  SHA will be recorded.
- Read-only P18 oracle: branch `codex/vp8l-profile-hybrid-clustering` at
  `c04bed7bf044dc610081ff1de0e43a2a579258bb`, worktree
  `/Users/lance/.codex/worktrees/7d78/webp`.
- Historical P20 binary SHA-256:
  `9aa8fa08fb2288335a98ff4a3d5f64a5a7922372f7c4c1b1212ed98b9b1a29f8`.
- Historical P18 binary SHA-256:
  `05b8421c86f3286667c5cffef35ffc2bff77f68d944063a954793e2b870e64c9`.

P22 will not merge or rebase `main`, P18, P20, or P21 after branch creation.
It will not cherry-pick any P20/P21 design, audit, runner, evidence, or report
commit. P22 owns its test oracle, census, runners, summarizers, and evidence.

## Exact A and B ownership

Both variants share P20 preparation, exact block-frequency ownership,
proposals, refinement, growth, complete-RIFF costing, strict single fallback,
and selected-only packed writing. A/B dispatch is private `cfg(test)` state at
the profile orchestration boundary, outside proposal, frequency, cost, growth,
and writer loops. Release code has no runtime switch, counter, or timer.

Variant A is the exact P20 retained-full-plan search. E and B are built as
complete `SpatialCostPlan` values. Their full plans remain live while the
refined candidate is derived and built. Initial selection retains the complete
winner and drops the losing full plans. During LowLatency growth, the selected
full plan, including its `PlanParts`, encoded prefix, and
`Vec<EntropyTables>`, remains live while the next growth candidate and its full
plan are built. An accepted candidate replaces the selected full plan. Final
spatial/single selection writes the already retained full spatial plan.

Variant B uses a private metric-only candidate record containing exactly:

- candidate kind (`E`, `B`, `R`, or `Split`);
- the owned `PlanParts` (assignments and exact group frequencies);
- the compact `PlanMetric` below.

Every B candidate is still built as the exact same `SpatialCostPlan`, so all
prefix construction, nested-map encoding, table construction, payload cost,
padding, overflow checks, and allocation checks execute at the same logical
point. Immediately after a successful build, a narrow private consuming
conversion moves its `PlanParts` and metric into the record. Consuming the
plan drops its encoded prefix and `Vec<EntropyTables>` without cloning
`PlanParts`. E, B, and refined metric records may coexist, but no successfully
costed full plan survives this conversion. Initial selection uses the same
`(riff_bytes, tie_rank)` ordering and moves the winning record.

During B growth, the selected metric-only record remains live while
`grow_once` derives the next `PlanParts`. The exact candidate full plan is
built, then immediately consumed into a metric-only record before the strict
RIFF comparison. A rejected record is dropped; an accepted record replaces
the selected record. Therefore no previous encoded prefix or entropy-table
vector remains live while a later full plan is built.

After the identical final spatial/single choice, B moves the winning
`PlanParts` into exactly one newly built writable `SpatialCostPlan` and writes
it. If single wins, B performs no final spatial materialization. This final
build uses the same prefix owner, `SpatialCostPlan::build`, and selected-only
writer as A. There is no alternate serializer.

## PlanMetric and equivalence

`PlanMetric` has four `usize` fields:

- `payload_bits`: exact planned VP8L payload bit length;
- `payload_bytes`: `ceil(payload_bits / 8)` before RIFF even-byte padding;
- `riff_bytes`: complete file length after VP8L chunk padding and RIFF framing;
- `group_count`: the number of exact group-frequency/table sets.

The consuming conversion reads these four values only after
`SpatialCostPlan::build` succeeds, and moves out the plan's own `PlanParts`.
Selection and growth stopping consult only `riff_bytes`, exactly as A does.
`payload_bits` and `payload_bytes` remain audit values, while `group_count`
proves that the metric describes the moved parts and supports lifetime census.

Equivalence follows from construction: A and B invoke the same complete plan
builder on the same prefix, profile, dimensions, map width, assignments, and
frequencies. The metric is copied from that successfully built plan, not
recomputed. Moving `PlanParts` cannot change its vectors. Dropping prefix and
tables cannot affect later proposal/refinement/growth operations, which read
only exact block frequencies and retained `PlanParts`. Final B materialization
repeats the deterministic builder on the selected parts and therefore must
recover the same metric, table codes, prefix, and bytes. Tests require metric
and byte identity rather than relying on this argument alone.

## Clone, allocation, lifetime, and error invariants

- Converting a successfully costed B plan consumes it and moves `PlanParts`;
  it performs no `PlanParts::clone` and no candidate-side allocation.
- Any assignment cloning already owned by deterministic `grow_once` remains
  unchanged. P22 adds no clone inside proposal, cost, reassignment, or growth.
- A keeps P20's clone behavior and full-plan lifetimes unchanged.
- B performs the same candidate plan allocations and checks as A, then frees
  candidate prefix/table storage earlier. The sole extra builder invocation is
  the declared final materialization, and only when spatial beats single.
- Candidate build failures propagate at the same candidate and with the same
  `EncodeError`. Final materialization propagates the existing allocation or
  output-size error category without remapping. Single-plan construction
  failure still invokes the complete base profile fallback before spatial
  search. No partial output is returned.
- Counter widths, checked arithmetic, allocation reservation policy, input
  limits, metadata/animation handling, and public error type remain unchanged.
- B's maximum simultaneously live full plans is expected to be one: the plan
  currently being costed or the final writable plan. A may retain three full
  initial plans and retains its selected full plan while a growth plan is
  built. The census must measure rather than assume these maxima.

## Test-only census definitions

Census state is private `cfg(test)`, scoped to one encode, disabled by default,
and passed or activated only outside inner loops. It is disabled for every
timed recovery and product measurement. Results aggregate by profile and
stage: `E`, `B`, `R`, `Growth`, and `FinalMaterialization`.

- `full_plans_built`: successful `SpatialCostPlan` builds at the stage.
- `final_materializations`: successful B builds at
  `FinalMaterialization`; A must report zero.
- `plan_parts_clones`: explicit orchestration-owned `PlanParts` clones. The B
  consuming conversion must contribute zero. Algorithm-owned assignment
  cloning inside `grow_once` is not a `PlanParts` clone and is not counted.
- `prefix_bytes`: logical prefix bytes retained by a live full plan,
  `ceil(prefix.bit_len / 8)`, sampled after exact plan construction.
- `table_entries`: live `EntropyTables` values, equal to the full plan's group
  count.
- `estimated_plan_heap_bytes`: logical prefix bytes plus
  `table_entries * size_of::<EntropyTables>()`, plus assignment capacity bytes
  and frequency capacity times `size_of::<EntropyFrequencies>()`. Vec headers
  and allocator overhead are excluded and reported as such.
- `maximum_live_full_plans`: high-water count of simultaneously live complete
  plans in one encode.
- `maximum_live_tables`: corresponding high-water live table entries.
- `maximum_live_prefix_bytes`: corresponding high-water logical prefix bytes.
- `maximum_live_estimated_heap_bytes`: corresponding high-water estimated
  full-plan heap bytes.

Full-plan lifetime accounting begins only after a plan build succeeds and ends
when it is consumed or dropped. A guard owned by each full plan updates the
high-water values; error paths cannot leave a live count. Metric-only records
are not full plans, but their `PlanParts` storage is included separately in
retained-record totals if reported. Corpus summaries must include per-profile,
per-stage totals and maxima for both variants.

## Fixed algorithm, semantics, and exclusions

E/B construction, clustering, exact histogram merge, one reassignment,
capacity proposals, split partition, rebuild, group compaction, group caps,
strict-growth stop, E/B/R/Split tie order, strict single fallback, prefix and
wire output, counter widths, allocation errors, and overflow errors are
identical to P20. Compact constructs no growth state; LowLatency uses the same
deterministic growth. Only the selected stream is written.

There is no image/size classifier, threshold tuning, concurrency, parameter
search, dependency, feature, public API, `Default` change, metadata change,
animation change, limit change, unsafe code, or thread. Release remains fixed
to A throughout Phase R. Only a passing recovery may replace release routing
with fixed B; after that replacement release contains no A switch, census,
counter, or timer.

## Locked corpora and evidence policy

The locked CLIC 1.0.0 validation corpus has 102 rows and manifest SHA-256
`9feb09f469753c43864011aa6f00cfc5ee1bd48da5aac8f9b16d105890e14f86`.
The locked recovery/product screen is its first 41 rows and has manifest
SHA-256
`474587feabe3178268b2eab6f7a166501d8ecc3d637a76bd412f4233dfa7b913`.
Expected Compact and LowLatency output totals are 599,398,064 and 601,400,998
bytes.

Durable design, report, concise JSON, provenance, runner/summarizer source, and
named invalidation notes live under
`experiments/vp8l-metric-only-plan-search`. Raw TSV, stderr, logs, run
directories, binaries, and `SHA256SUMS` remain in explicit external output
directories and are never force-added. Negative, invalid, superseded, and
outlier-bearing runs are preserved. A target-path rebuild may change binary
SHA, but one rebuilt SHA remains fixed across every phase of its replay.

## Phase R gates and hard stop

1. Before B exists, transplanted A must reproduce the locked 102-image totals
   and P18 per-profile stream hashes for 204/204 streams. Failure stops P22.
2. Unit/property tests must prove: consumed-plan metric identity for payload
   bits, payload bytes, RIFF bytes, and group count; rebuilt selected parts
   produce byte-identical output and metric; overflow/allocation errors retain
   their categories; and A/B identity for single/spatial ties, E/B/R/Split
   winners, forced `SinglePlan` fallback, and both profiles.
3. One non-timed locked 102-image run must prove A/B/public/P18 byte identity
   204/204; exact planner/writer/selector/fallback denominators; Compact growth
   and growth state 0/0; LowLatency growth 336/336; expected aggregates; and
   every per-profile/stage census count and high-water definition above.
4. One final test binary then runs the locked 41-image recovery screen. Inputs
   are preloaded, exactly one unscored warmup precedes three retained
   forward/reverse/forward interleaved full end-to-end A/B rounds, every sample
   is retained, and no valid rerun is permitted. LowLatency B versus A must
   improve the independent aggregate encode median by at least 3.0% with 0/41
   per-image median regressions. Compact aggregate regression must be at most
   1.0%, with every per-image regression reported. A/B output identity must be
   82/82 and every stderr file empty.

Any Phase R failure immediately rejects the experiment. Product release
routing, product Phase A, product screen, formal, and final gates are then
prohibited. P22 must commit a negative report and reproducer and leave release
routing on A.

## Product gates, only after recovery passes

1. Remove A production routing and every timing/census switch from release.
   Fixed B becomes the only product behavior. Lock the final validation binary
   and rerun 102-image Phase A from scratch: exact P18 identity, expected
   aggregates, 0/102 images above latest-main/E37 control by more than 2%, all
   planner/writer/selector/fallback denominators, Compact no growth/state, and
   LowLatency growth 336/336.
2. With that unchanged binary, run the full 41-image product screen against
   latest-main/E37 same-binary controls. Each profile must independently
   improve aggregate encode time by at least 50%, have 0/41 per-image encode
   regressions, no aggregate byte growth, no image above control by more than
   2%, Rust and pinned-C decode regression below 1%, RSS increase below both
   64 MiB and 5%, and project/pinned-C exactness for all six layouts. A hard
   failure stops formal.
3. Only after the screen passes, run locked 102x5 formal timing with the same
   binary: one warmup, then all five retained F/R/F/R/F rounds. Each profile
   must improve independent median encode time by at least 50%, have 0/102
   per-image median regressions, and have candidate median at most 7.1 seconds
   for Compact and 6.9 seconds for LowLatency.
4. Only after formal passes, require 102-image all-layout project/pinned-C
   exactness, product/no-product `Default` byte identity 102/102, stable
   default and relevant workspace all-target tests, all-target Clippy with
   warnings denied, formatting, rustdoc with warnings denied, doctests, and
   dependency/API/unsafe/thread/module/lifetime/resource/rlib/binary audits.
5. From the repository root and an isolated target, replay mechanism, product
   Phase A, screen, formal, correctness, identity, and quality. The replay's
   rebuilt binary SHA must remain unchanged within every replay phase.

Only the installed stable host target is permitted. Passing every gate
recommends promotion without integrating `main`; any hard failure recommends
rejection and stops all later phases.
