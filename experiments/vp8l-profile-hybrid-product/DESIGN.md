# P20 product migration: profile-specialized exact-cost hybrid

This design is frozen before any P20 corpus measurement. P20 manually rebuilds
the minimum production form of the passing P18 architecture on latest main. It
does not merge, rebase, or cherry-pick P18 and does not import P18's research
feature, trace/census state, broad benchmark hooks, raw output, or research-only
API.

## Provenance and fixed scope

- Task: `019f8a85-c530-79d2-af1f-2b54105574be`
- Root task: `019f8321-035e-7211-8f53-987e18891c8c`
- Branch: `codex/vp8l-profile-hybrid-product`
- Worktree: `/Users/lance/.codex/worktrees/5020/webp`
- Base, creation `HEAD`, local `main`, and merge-base:
  `66c15f11c0cd63a7e5ad80ffbe7553e6f68ec569`
- Read-only P18 oracle: `codex/vp8l-profile-hybrid-clustering` at
  `c04bed7bf044dc610081ff1de0e43a2a579258bb`

P19's creation-base race remains recorded under `invalidated-runs` and supplies
no implementation or measurement evidence.

## Production ownership and lifetime

The existing public `FastDecodeCompact` and `FastDecodeLowLatency` variants
keep their API and container orchestration. Both enter one private product
writer and create exactly one prepared value. Preparation owns validation,
dimensions, alpha state, the existing E37 token order, fast prefix inputs, and
the exact single-group frequencies. Metadata wrapping remains outside this
planner exactly as on the base.

Private production modules are split by owned invariant, each targeted below
500 production lines:

- exact block frequencies own the 1,049 per-block counts, block geometry, E/B
  summaries, support checks, and exact block-to-model payload evaluation;
- proposal clustering owns deterministic seed ranking, assignment, empty-block
  fill, and ascending group compaction;
- plan state/refinement owns assignments plus rebuilt group frequencies,
  exactly one global reassignment, and the LowLatency split transition;
- exact spatial cost owns the nested map, five-table headers, token code and
  extra bits, padding, complete-RIFF arithmetic, and selected-only packed
  serialization with planned-bit/payload/RIFF assertions;
- profile orchestration owns candidate order, complete-RIFF tie rules, strict
  single fallback, and the one selected write.

Dependencies are directional from orchestration to plan/refinement, exact
frequencies/proposals, and cost/write. Frequency and proposal owners do not call
the writer. Compact constructs no self-cost cache, split partition, growth
candidate, or growth trace. LowLatency constructs at most one growth candidate
per step; a rejected candidate is dropped and an accepted candidate replaces
the prior growth plan. Exact counters outlive planning and are dropped after
the selected stream is written.

## Fixed algorithm and ties

E selects each channel's exact dominant symbol, smallest symbol on a tie, then
maps it to the existing 32-symbol signature bin. B sums the same exact counts
into the eight existing bins, selects greatest mass, and uses the smallest bin
on a tie. Both retain the existing profile block size, group cap, weighted seed
ranking, distance, assignment, empty fill, and compaction rules. There is no
third signature.

Both E and B are charged by an exact spatial plan: fast prefix, no-cache and
meta-Huffman flags, block bits, complete nested group map, all five adaptive
Huffman headers per group, weighted token code lengths, length/distance extra
bits, byte rounding, VP8L padding, and complete RIFF bytes. E wins an E/B tie.

Starting from the E/B winner, exactly one global reassignment evaluates every
non-empty block under current group code lengths. A model missing any required
symbol is infeasible. Equal cost retains the block's original group; otherwise
the lowest group id wins. Empty groups compact in ascending old-group order and
exact group frequencies rebuild once. Spatial selection compares E, B, and
refined by complete RIFF bytes; ties retain E, then B, then refined.

Compact stops there. It must report and perform growth 0/0 and construct no
capacity-growth state.

LowLatency starts from that same exact winner. For exact histogram `h`, `C(h)`
is the exact five-table header plus payload cost, including extra bits, and
`merge_penalty(a,b) = C(a+b) - C(a) - C(b)`. The combined histogram uses the
union of supports; there are no pseudocounts.

Each deterministic LowLatency growth step:

1. Among blocks in a group with at least two non-empty block histograms, select
   greatest current-model payload regret versus its self-model, then smallest
   raster block index.
2. In that source group select seed two by greatest support-safe combined
   histogram merge penalty from seed one, then smallest raster block index.
3. Seed one retains the old group and seed two creates the new group. Other
   non-empty source-group blocks choose the smaller merge penalty; a tie stays
   with seed one. Other groups and empty blocks retain their assignments.
4. Rebuild exact frequencies, perform exactly one shared global reassignment,
   compact ascending empty group ids, and rebuild once.
5. Fully cost the candidate and accept only a strict complete-RIFF decrease.

The first tie or increase stops growth. Growth also stops at the existing
LowLatency group cap, when no source is splittable, or when reassignment equals
the prior accepted assignments. Final candidate order is E, B, refined, split;
ties keep the earliest. Exact single wins every complete-RIFF tie. A failure to
construct the exact single plan strictly falls back to the complete base E37
profile path. Only the selected stream is serialized.

## Fixed exclusions and semantics

The implementation is safe Rust and single-threaded. It adds no image IDs,
corpus thresholds, runtime classifier, parameter search, second resolution,
third signature, dependency, feature, public API, thread, or unsafe block.
`Default` bytes and behavior do not change. Output is ordinary standard VP8L.
Input limits, allocation failures, metadata bytes/flags, animation behavior,
and error mapping remain those of the base.

## Predeclared validation and stop policy

The locked 102-row manifest must hash to
`9feb09f469753c43864011aa6f00cfc5ee1bd48da5aac8f9b16d105890e14f86`;
its first 41 rows must hash to
`474587feabe3178268b2eab6f7a166501d8ecc3d637a76bd412f4233dfa7b913`.
Generated measurements, logs, per-run directories, and checksums live in an
explicit external output directory.

Phase A must prove image-by-image product/P18 candidate byte identity for both
profiles, expected aggregates 599,398,064 and 601,400,998 bytes unless a proven
legal main change explains a difference, latest-main same-binary controls,
planner/writer payload bits/bytes/RIFF identity, E/B and final selectors, strict
single fallback, selected public output, Compact growth 0/0, LowLatency growth
336/336, aggregate rate no worse than control, and 0/102 above control by more
than 2%.

Only a passing Phase A permits the locked 41-image screen. One unchanged final
binary preloads inputs, performs one warmup, then three forward/reverse/forward
interleaved rounds, retaining every sample. Independently for both profiles it
must achieve at least 50% aggregate encode improvement, 0/41 per-image encode
regressions, aggregate bytes no worse than control, 0/41 above +2%, Rust and
pinned-libwebp decode regression below 1%, RSS increase below both 64 MiB and
5%, and complete RGBA identity in both decoders. Any failure is archived and
stops later expensive phases.

Only a passing screen permits locked 102-image formal measurement: one warmup
then five forward/reverse/forward/reverse/forward rounds from the same final
binary, all samples retained. Each profile must exceed 50% independent median
improvement, have 0/102 per-image median regressions, and have candidate
absolute median below 7.1 s for Compact and below 6.9 s for LowLatency.

Final gates are all required layouts for 102 images with project and pinned-C
exact decode; same-source product/no-product `Default` 102/102 byte identity;
stable default and relevant-configuration workspace all-target tests; Clippy
all targets with warnings denied; formatting; rustdoc with warnings denied;
doctests; API/dependency/unsafe/thread/module/resource audit; binary and rlib
size; worst-case memory; and a repository-root one-command isolated-target
replay of Phase A, screen, formal, correctness, identity, and quality. A
target-path-induced binary hash change is recorded; every phase within a replay
must use one unchanged rebuilt SHA.

Passing every gate recommends promotion. Any hard failure recommends rejection,
stops later expensive phases, and retains the negative report and reproducer.
