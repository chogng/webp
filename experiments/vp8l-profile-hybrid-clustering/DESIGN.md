# P18 predeclared profile-specialized exact-cost hybrid

This design was frozen before any P18 corpus-rate output. P18 has exactly two
planner paths selected only by the existing public lossless profile at the
planner responsibility boundary.

## Shared ownership and lifetime

Both profiles call the same feature-private encoder entry point and construct
one actual `Prepared` value. That value owns validation, dimensions, alpha
state, the existing E37 tokenization, the same token order, the fast prefix,
and the exact `SinglePlan` input frequencies. One `ExactBlockFrequencies`
instance is then collected from those prepared tokens and owns the 1,049 exact
per-block counters used by E, B, reassignment, costing, and (only for
LowLatency) capacity growth. A copy belongs to its starting spatial block.

The exact counter is built once per encode, before proposals. E and B plans,
their exact costs, and the refined winner may coexist for audit and deterministic
selection. Compact never allocates or constructs split candidates, regret
caches, partitions, or growth traces. LowLatency may construct one split
candidate at a time; a rejected candidate is dropped immediately, while an
accepted candidate replaces the previous growth state. The counter outlives
planning and is released before the selected stream is returned. Serialization
uses the same selected-only packed token writer. The losing spatial and single
main token streams are not serialized.

## Shared exact proposals and cost

E uses each channel's exact dominant symbol, with the smallest symbol on a tie,
mapped to the existing fixed 32-symbol bin. B sums the same exact counts into
the existing eight fixed bins, choosing greatest mass and then the smallest bin.
The existing deterministic seed ranking, assignment, group cap, empty fill, and
compaction rules are retained. No third signature exists.

`SpatialCostPlan` fully charges the fast prefix, no-cache/meta-Huffman flags,
block bits, complete nested group map, all five adaptive Huffman headers for
every group, weighted token code lengths, length/distance extra bits, byte
rounding, VP8L padding, and complete RIFF bytes. Its planned payload bits,
payload bytes, and RIFF bytes must equal the actual writer for every audited
candidate.

E wins an E/B complete-RIFF tie. Starting from that E/B winner, exactly one
global code-length reassignment evaluates each non-empty block under the
current group code lengths. Absent symbols are infeasible. Equal cost retains
the original group; otherwise the lowest group id wins. Empty groups are
compacted in ascending old-group order and exact group frequencies are rebuilt
once. Initial spatial selection compares E, B, and refined by complete RIFF;
ties retain E, then B, then refined.

## Compact path

Compact returns the complete-RIFF winner of E, B, and refined. It performs no
capacity-growth attempt and constructs no growth state. Complete-file selection
then compares that spatial winner with the exact `SinglePlan`; single wins a
complete-RIFF tie. Only the selected stream is written.

## LowLatency capacity growth

LowLatency begins at the same E/B/refined winner. For exact histogram `h`,
`C(h)` is the exact five-table header plus payload bit cost, including
length/distance extra bits. The support-safe distance is

`merge_penalty(a,b) = C(a+b) - C(a) - C(b)`.

The combined histogram contains the union of both supports; there are no
pseudocounts and no absent-symbol evaluation under another block's codebook.
Each iteration is deterministic:

1. Among blocks whose source group has at least two non-empty block histograms,
   choose greatest exact coding regret, then smallest raster block index.
   Regret is payload cost under the current group model minus self-model
   payload cost; group header cost is excluded from this ranking only.
2. In that source group, choose seed two by greatest support-safe combined-
   histogram merge penalty from seed one, then smallest raster block index.
3. Seed one retains the old group and seed two creates the new group. Other
   non-empty source-group blocks choose the smaller merge penalty; a tie stays
   with seed one. Other groups and empty blocks keep their assignments.
4. Rebuild exact frequencies, perform exactly one global reassignment with the
   shared tie rules, compact ascending empty group ids, and rebuild once.
5. Fully cost the candidate with `SpatialCostPlan`. Accept only a strict
   decrease in complete RIFF bytes.

The first tie or increase is rejected and stops growth. Growth also stops at
the existing LowLatency profile group cap, when no block is splittable, or when
post-reassignment assignments equal the prior accepted assignments. Final
spatial selection compares E, B, refined, and the final accepted split; ties
retain E, then B, then refined, then split. Exact single wins every complete-
RIFF tie. There is no second-resolution portfolio.

## Fixed exclusions

There is no image id, corpus threshold, runtime classifier, random input,
parameter search, concurrency, unsafe code, new dependency, third signature,
public API change, `Default` change, metadata change, animation change, or error
semantics change. Output remains ordinary standard VP8L.

## Gates and stop policy

The locked 102-row manifest must hash to
`9feb09f469753c43864011aa6f00cfc5ee1bd48da5aac8f9b16d105890e14f86`.
Phase A must prove, per image and profile, planned bits/payload bytes/full RIFF
equal writer, E/B selector equals actual smaller stream, final selector equals
the legal candidate set plus strict single fallback, and public output equals
the selected stream. Expected unchanged-wire totals are Compact 599,398,064 B
and LowLatency 601,400,998 B, both 0/102 above the same-binary E37 control by
more than 2%. Any difference is investigated without changing this design.
Rate, tail, and every exactness denominator must pass before screen.

The fair 41-image screen uses one final binary that produces both control and
candidate streams from the same prepared/token/single/prefix/writer owner.
Inputs are preloaded; one warmup precedes three forward/reverse/forward rounds.
Each profile must improve aggregate encode median by at least 10%, have 0/41
per-image encode median regressions, aggregate bytes no larger than control,
no image above +2%, Rust and pinned-libwebp decode regression at most 1%, RSS
increase below both 64 MiB and 5%, and exact RGBA under both decoders. A binary
change invalidates the entire prior suite by name.

Only a fully passing screen permits locked 102-image x5 formal measurement.
Formal requires Compact absolute median at most 7.1 s, LowLatency at most
6.9 s, and 0/102 per-image median regressions, with every sample and outlier
retained. Only then are Default byte identity, 102-image project/pinned-C full
exactness, feature/default stable workspace tests, all-target Clippy with
warnings denied, fmt, docs/doctest, and resource/module-boundary audits run as
final gates. Any failed gate stops later expensive phases and is archived.
