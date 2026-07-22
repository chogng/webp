# P23 analytic exact-cost planning and selected-only materialization

This design is frozen before the authorized P20 production transplant, before
P23 implementation, and before any P23 corpus timing. P23 isolates the cost of
serializing each rejected spatial candidate. It does not change proposal,
refinement, growth, selection, fallback, profile, or wire rules.

## Identity and controlled provenance

- Task: `P23 independent VP8L analytic exact-cost / selected-only
  materialization experiment`
- Root task: `019f8321-035e-7211-8f53-987e18891c8c`
- Branch: `codex/vp8l-analytic-exact-cost`
- Worktree: `/Users/lance/.codex/worktrees/fc25/webp`
- Base, creation `HEAD`, creation local `main`, and creation merge-base:
  `76c9aa39e35534b847f2cb980cb0037c4e6be785`
- Both creation ancestry checks succeeded. The worktree was clean and detached
  before it was attached to the branch above.
- Read-only P20 product source:
  `/Users/lance/.codex/worktrees/5020/webp` at
  `cebc0981c23d7b4e719b1491ee83907560e0bd63`. Only production commit
  `67bd04274a60fe81fcd13cc2702d75e3fd0553a4` may be cherry-picked after this
  design commit. Its resulting P23 SHA will be recorded.
- Read-only P18 oracle: `/Users/lance/.codex/worktrees/7d78/webp` at
  `c04bed7bf044dc610081ff1de0e43a2a579258bb`.
- Read-only rejected P22 mechanism/evidence:
  `/Users/lance/.codex/worktrees/c5fc/webp` at
  `4b80999fb95c816c52a4bcf4da62d1d52f780f3c`. No P22 code or commit may be
  cherry-picked.
- Historical P20 binary SHA-256:
  `9aa8fa08fb2288335a98ff4a3d5f64a5a7922372f7c4c1b1212ed98b9b1a29f8`.
- Historical P18 binary SHA-256:
  `05b8421c86f3286667c5cffef35ffc2bff77f68d944063a954793e2b870e64c9`.

P23 does not merge or rebase `main`, P18, P20, P21, or P22 after branch
creation. It owns its analytic implementation, test oracle, census, runners,
summarizers, invalidations, and evidence.

## Exact A and B

A is the exact P20 retained-full-plan search. Every E, B, R, and attempted
LowLatency Split candidate calls `SpatialCostPlan::build`. That builder writes
the fast prefix, spatial flags, nested group map, and five adaptive table
headers per group into a temporary `BitWriter`, retains the prefix and a
`Vec<EntropyTables>`, analytically adds token payload bits, and retains the
winning complete plan for the existing selected-only packed writer.

B owns, for each candidate, only its kind, `PlanParts`, and a four-field exact
metric: `payload_bits`, `payload_bytes`, complete `riff_bytes`, and
`group_count`. No B candidate constructs a `SpatialCostPlan`, candidate prefix
byte vector, or retained candidate `Vec<EntropyTables>`. B preserves P20's
exact block frequencies, E/B proposals, one reassignment, LowLatency growth,
ties, strict stops, single fallback, profiles, and final wire.

After the unchanged spatial/single comparison, a spatial B winner moves its
`PlanParts` into exactly one existing `SpatialCostPlan::build` and uses the
existing selected-only packed writer. If single wins, B performs no final
spatial materialization. A/B dispatch and census are private `cfg(test)`,
outside hot loops, disabled during timing. Release remains fixed A throughout
recovery. Only a passing recovery permits replacing release with fixed B;
promotion then removes A production routing and every switch, census, and
timer.

## Frozen analytic formulas

All arithmetic below uses `checked_add`, `checked_mul`, `usize::try_from`, and
the existing `EncodeError::output_size_overflow` mapping unless an existing
allocation is being made, in which case reservation failure remains
`EncodeError::allocation_failed`.

### Fast and spatial prefix

The current fast VP8L prefix is 44 bits: signature/version, two 14-bit
dimensions, alpha, subtract-green transform tag, and transform terminator. B
may compute this once per prepared image as a bit count; it does not allocate
or serialize a candidate prefix. Each spatial candidate then adds:

`1 no-cache bit + 1 meta-Huffman-present bit + 3 block-size bits`.

The nested group-map dimensions and RGBA bytes remain exact P20 behavior. For
assignments `a[0..M)`, allocate `4*M` bytes with checked multiplication and
exact reservation, append `[0, group, 0, 0]` in assignment order, and call
`collect_entropy_tokens(rgba, map_width, false, false, 0)`. This preserves the
same tokenization, frequencies, copy formation, and allocation/error checks.

The nested map adds one no-cache bit, five adaptive table-header costs, and
the exact payload cost of its collected tokens/frequencies. The analytic path
builds the same five `EncodingTable` values, uses them for payload widths, and
drops the map tables immediately after the nested-map cost is complete.

### Adaptive table header

The current `write_adaptive_table` has one route for every legal frequency
shape: it calls `prepare_adaptive_table` and then `write_normal_table`. Empty
frequencies are normalized to symbol zero; one-symbol inputs still use the
normal representation because simple codes cannot cover all legal green
symbols. There is no adaptive simple or alternate representation to model.

`write_normal_table` has this exact layout:

`1 normal marker + 4 code-length count + 19*3 code-length entries +`
`1 no-shortening flag + N*4 fixed-width encoded lengths`.

Therefore `normal_table_header_bits(N) = 63 + 4*N`, checked. The invariant is
centralized in the narrow adaptive-table owner and tested against the real
`BitWriter` for every legal alphabet length used here and varied empty,
one-symbol, sparse, dense, tied, and maximum-counter frequency shapes. The
analytic table is the exact `EncodingTable` returned by the existing
`prepare_adaptive_table`; tests compare all canonical `(code,width)` entries
with the real writer/builder result.

### Symbol and copy payload

For a frequency vector `f` and matching encoding table widths `w`, symbol
payload is the checked sum `sum_i usize(f_i) * usize(w_i)`. This is applied to
green up to `green_len`, red, blue, alpha, and distance exactly once.

For each length prefix `p` in `0..24`, add
`green[256+p] * length_extra_width(p)`, where the width is zero for `p < 4`
and `(p-2)>>1` otherwise. Sum all green copy-prefix frequencies in `u64`.
Independently sum all distance frequencies in `u64`; unequal copy counts are
an output-size overflow. The fixed distance-one representation uses
`vp8l_prefix(121, 40)` and adds
`copy_count * distance_extra_width` with checked conversion and multiplication.
Cache, literal, length-prefix, and distance-prefix code widths are already in
the five ordinary symbol sums; only their extra bits are added separately.

For each main entropy group, B calls the same adaptive table builder for the
same five frequency arrays, adds `normal_table_header_bits` for each, adds the
symbol and copy-extra payload above, and drops those tables before processing
the next group. It never retains a cross-group table vector.

### Complete metric

Let `P` be the checked sum of the 44-bit fast prefix, five spatial-control
bits, the complete nested-map cost, all five main table-header costs for every
group, and all main-group symbol/copy-extra payload costs. Then:

- `payload_bits = P`;
- `payload_bytes = checked_add(P, 7) / 8`;
- `padded_payload_bytes = payload_bytes + (payload_bytes & 1)`;
- `riff_size_field_plus_header = 12 + padded_payload_bytes`, which must fit
  `u32` exactly as in P20;
- `riff_bytes = riff_size_field_plus_header + 8`;
- `group_count = parts.frequencies.len()`.

These are the exact P20 `SpatialCostPlan` formulas, including final-byte zero
padding, even VP8L chunk padding, and RIFF framing.

## Ownership, modules, and error invariants

The existing exact spatial-cost module continues to own the writable full
plan, nested-map construction invariant, complete metric, and selected writer.
A narrow analytic-cost value/function in that responsibility owns only
`PlanParts` inspection and transient table construction. The adaptive-table
module owner exposes the narrow checked normal-header count next to the real
writer invariant. Profile orchestration owns candidate kind, candidate
`PlanParts`, A/B selection, tie ranks, growth, final materialization, and
test-only dispatch/census. Dependencies remain directional: clustering and
refinement never depend on costing or writing.

B's map RGBA/tokens/frequencies live only through nested-map costing. Its five
map tables then drop. Each main group's five tables live only through that
group's analytic payload count and then drop. Candidate `PlanParts` and metric
survive selection; no candidate prefix or table vector does. The one winning
full plan owns final prefix/tables until writing.

B removes temporary candidate `BitWriter` allocation/resizing and retained
cross-group table-vector allocation. It introduces no new allocation site and
preserves every semantically required allocation and overflow check for map
RGBA, tokenization, adaptive lengths/canonical tables, arithmetic, final plan,
and output. Removing a non-output temporary allocation can allow B to proceed
where A would have reported allocation failure, but it cannot turn malformed
state or arithmetic overflow into output, and the final writable plan still
performs the existing allocation/error mapping before bytes are returned.
Tests require unchanged public categories for forced fallback, analytic
overflow, final materialization allocation/overflow passthrough, and no
partial output. Metadata, animation, dimensions, limits, and strict single
fallback remain outside and unchanged.

## Locked manifests and census

The locked CLIC 1.0.0 validation corpus has exactly 102 rows and manifest
SHA-256
`9feb09f469753c43864011aa6f00cfc5ee1bd48da5aac8f9b16d105890e14f86`.
The recovery/product screen is exactly its first 41 rows and has manifest
SHA-256
`474587feabe3178268b2eab6f7a166501d8ecc3d637a76bd412f4233dfa7b913`.
Expected Compact and LowLatency totals are 599,398,064 and 601,400,998 bytes.

One non-timed 102-image audit reports A and B per profile and stage `E`, `B`,
`R`, `Growth`, and `FinalMaterialization`:

- candidate attempts and successful candidate metrics;
- adaptive table builds (five per nested map and five per main group);
- nested-map tokenizations;
- candidate `BitWriter` logical bits, final bytes, capacity resizes or
  allocation events measurable without changing allocator semantics;
- retained entropy-table entries;
- analytic checked-bit additions/multiplications/conversions;
- final full-plan materializations;
- maximum live logical prefix bytes, table entries, table-code entries, map
  RGBA bytes, map tokens, and total explicitly estimated plan storage, with
  allocator overhead excluded and labeled.

Denominators are every attempted E/B/R and every attempted Split, including a
rejected stopping Split. Metric equality requires `payload_bits`,
`payload_bytes`, `riff_bytes`, and `group_count` for every successful A/B
candidate comparison, not only winners. Compact planner/writer is 306/306 and
growth 0/0. LowLatency planner/writer is 408/408 and growth 336/336. E/B
selectors, final selectors, strict fallback, public/A, A/B, and P18 identities
are each 204/204 as applicable. Census is disabled for timing.

Durable design, report, concise JSON summaries, provenance, runner/summarizer
source, and named invalidations live under
`experiments/vp8l-analytic-exact-cost`. Raw TSV, stderr, logs, run directories,
binaries, and `SHA256SUMS` stay in explicit external directories and are never
force-added. Every valid, failed, invalidated, and outlier-bearing attempt is
preserved; no valid timing screen may be rerun.

## Mechanism and exactness gates

Before B exists, transplanted A must reproduce 599,398,064 / 601,400,998 bytes
and 204/204 P18 stream size plus hash identity. Failure stops P23.

Unit/property tests must prove:

1. analytic adaptive-header plus payload bits equal the real `BitWriter` for
   empty, one-symbol, sparse, dense, legal green lengths, tied and maximum
   constructible counters, copy prefixes/extras, and overflow-adjacent checked
   arithmetic;
2. analytic canonical widths/tables equal the existing real builder tables;
3. complete analytic metric equals `SpatialCostPlan` for single/spatial ties
   and E/B/R/Split winners under both profiles;
4. final rebuild writes identical bytes and preserves selection, fallback,
   error categories, and single-winner no-materialization behavior.

The locked non-timed 102-image audit must prove metric equality at every
candidate denominator, A/B/public/P18 identity 204/204 with the fixed totals,
the planner/writer/growth/selector/fallback denominators above, and complete
census totals/high-water values. Any metric, selection, wire, or error drift
rejects P23 before timing.

## Single valid recovery screen

Use one final release test binary. Preload inputs; perform exactly one unscored
warmup; retain exactly three forward/reverse/forward interleaved A/B rounds.
There is no valid rerun. Every sample and outlier remains.

- LowLatency B versus A aggregate encode improvement must be at least 5.0%,
  with 0/41 per-image median regressions.
- Compact aggregate regression must be below 0.5%; every per-image regression
  is reported.
- A/B output identity must be 82/82 and stderr must be empty.

Any failure stops the experiment, keeps release fixed to A, prohibits every
product phase, and requires a committed negative report and reproducer.

## Product gates only after recovery passes

1. Remove A production routing and all switches, census, and timers. Fix
   release to B and lock one final binary. Rerun 102-image Phase A from scratch:
   fixed totals, P18 identity, 0/102 above control +2%, all exact denominators,
   Compact no growth state, and LowLatency growth 336/336.
2. With that same binary, run the full 41-image product screen against
   latest-main/E37 controls. Each profile must improve aggregate encode by at
   least 50%, have 0/41 encode regressions, no aggregate byte growth, 0 images
   above control +2%, Rust and pinned-C decode regression below 1%, RSS
   increase below both 64 MiB and 5%, and exactness for all six layouts.
3. Only after the screen passes, run locked 102x5 formal timing: one warmup and
   five retained F/R/F/R/F rounds. Each profile must improve independent median
   encode by at least 50%, have 0/102 per-image median regressions, and have
   candidate median below 7.1 seconds Compact and 6.9 seconds LowLatency.
4. Require 102-image all-layout project/pinned-C exactness; product/no-product
   `Default` 102/102 byte identity; stable default and relevant workspace
   all-target tests; Clippy all targets with warnings denied; formatting;
   rustdoc with warnings denied; doctests; dependency/API/unsafe/thread/module/
   resource/rlib/binary audit; and isolated-target replay of mechanism, Phase
   A, screen, formal, correctness, identity, and quality.

Only the installed stable host target is permitted. Passing every gate
recommends promotion on this clean branch without integrating `main`. Any hard
failure recommends rejection and stops all later phases.
