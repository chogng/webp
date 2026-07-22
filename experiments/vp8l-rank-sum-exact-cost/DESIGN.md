# P24 allocation-free rank-sum exact-cost experiment

This design is frozen before the authorized P20 production transplant, before
P24 implementation, and before any P24 corpus timing. P24 isolates the cost of
constructing adaptive/canonical Huffman tables while evaluating rejected
spatial candidates. It does not change tokenization, proposals, refinement,
growth, selection, fallback, profiles, or the final VP8L wire.

## Identity and controlled provenance

- Task: `P24 independent VP8L allocation-free rank-sum exact-cost experiment`
- Root task: `019f8321-035e-7211-8f53-987e18891c8c`
- Branch: `codex/vp8l-rank-sum-exact-cost`
- Worktree: `/Users/lance/.codex/worktrees/a5b2/webp`
- Base, creation `HEAD`, creation local `main`, and creation merge-base:
  `230ce0bd1c201d2687261d97a525cec8f91aa215`
- Both creation ancestry checks succeeded. The worktree was clean and detached
  before it was attached to the branch above.
- Read-only P20 product source:
  `/Users/lance/.codex/worktrees/5020/webp` at
  `cebc0981c23d7b4e719b1491ee83907560e0bd63`. Only production commit
  `67bd04274a60fe81fcd13cc2702d75e3fd0553a4` may be cherry-picked after this
  design commit.
- Read-only P23 exact oracle/evidence:
  `/Users/lance/.codex/worktrees/fc25/webp` at
  `8ff5ac492ababf1252fb51de74db1f28348b4b16`. P24 may selectively reconstruct
  its analytic test oracle but will not cherry-pick P23 evidence or claim P23
  measurements as P24 evidence.
- Read-only P18 oracle: `/Users/lance/.codex/worktrees/7d78/webp` at
  `c04bed7bf044dc610081ff1de0e43a2a579258bb`.
- Historical P20 binary SHA-256:
  `9aa8fa08fb2288335a98ff4a3d5f64a5a7922372f7c4c1b1212ed98b9b1a29f8`.
- Historical P18 binary SHA-256:
  `05b8421c86f3286667c5cffef35ffc2bff77f68d944063a954793e2b870e64c9`.

P24 does not merge or rebase `main`, P18, P20, or P23 after branch creation.
It owns its implementation, test oracle, census, runners, summarizers,
invalidations, measurements, and report.

## Exact A, O, and B

A is P20 retained-full-plan search. Every E, B, R, and attempted LowLatency
Split candidate builds a `SpatialCostPlan`, candidate prefix, and complete
cross-group entropy-table vector. Release remains fixed A through recovery.

O is the P23-style test-only analytic oracle. It owns only candidate kind,
`PlanParts`, and exact metric; it does not own a candidate `BitWriter`, prefix
bytes, or retained table vector. It still calls the real adaptive/canonical
table builder for exact code widths. O is excluded from timing and release.

B owns only candidate kind, `PlanParts`, and exact metric. No B candidate owns
or constructs a `SpatialCostPlan`, prefix byte vector, `EncodingTable`,
length/code `Vec`, canonical code, or cross-group table vector. Its main-group
histogram costs use the rank-sum formula below and must not call
`prepare_adaptive_table`, `canonical_table`, or `build_tables`. Its nested map
retains the identical logical `[0, group, 0, 0]` input, dimensions,
`collect_entropy_tokens` behavior, no-cache bit, five headers, token payload,
copy-prefix extras, and fixed distance-one extra accounting. It may use the
same rank-sum primitive for those five nested-map histograms; tokenization is
not specialized or changed.

A/O/B dispatch and census are private `cfg(test)`, outside hot loops and
disabled for timing. If recovery passes, production A routing is removed and
release becomes fixed B with no switch, counter, or timer. Only the selected
spatial winner then calls the existing real builder/writer once; a winning
single plan performs no spatial materialization.

## Frozen rank-sum formula and tie proof

For a legal frequency vector of length `N <= 296`, let `k` be the number of
nonzero frequencies.

- `k = 0`: the real canonical-table special path consumes zero data-symbol
  bits. Exact data-symbol cost is zero.
- `k = 1`: the sole nonzero symbol receives width zero. Exact data-symbol cost
  is zero.
- `k >= 2`: let `L = floor(log2(k))`, `base = 2^L`, and
  `m = 2 * (k - base)`. The deterministic balanced adaptive table assigns
  width `L + 1` to exactly the `m` least-frequent nonzero symbols and width `L`
  to the remaining `k - m`. If `S` is the checked sum of all nonzero
  frequencies and `R` the checked sum of the `m` least nonzero frequencies,
  exact data-symbol bits are `L * S + R`.

The long-count derivation follows Kraft equality. With `x` codes of width
`L + 1` and `k - x` of width `L`, multiplying
`(k-x)/2^L + x/2^(L+1) = 1` by `2^(L+1)` gives
`2k - x = 2^(L+1)`, hence `x = 2*(k-2^L) = m`.

Frequency ties cannot alter the weighted sum. If a tie of value `t` straddles
the selection boundary, every legal choice selects the same number `q` of
members of that tied class and therefore contributes exactly `q*t`; symbols
outside the tied class are unchanged. Symbol-index order can affect canonical
codes but not code widths multiplied by frequencies, so rank selection is
wire-cost exact without reproducing tie order.

The implementation uses checked conversion, addition, and multiplication and
maps arithmetic failure to the existing output-size-overflow category. It
copies nonzero counters into a fixed stack scratch of
`MAIN_GREEN_ALPHABET_SIZE = 296` entries and uses
`select_nth_unstable` or another frozen allocation-free selection operation.
It has no unsafe code and no heap fallback. The legal input slice is rejected
if its length exceeds the scratch capacity; tests prove every production
alphabet fits and the scratch high-water mark never exceeds 296.

## Header, copy, payload, and RIFF formulas

The current normal adaptive-table header is always
`1 + 4 + 19*3 + 1 + 4*N = 63 + 4*N` bits, checked, for every legal shape.
The `k=0/1` data-symbol special cases do not remove or shorten that header.

For each entropy group B adds five checked headers and rank-sum symbol payload
for green through the active `green_len`, red, blue, alpha, and distance.
Length-prefix extra bits are
`sum green[256+p] * extra_width(p)` for `p in 0..24`, where the width is zero
for `p < 4` and `(p-2)>>1` otherwise. The checked sum of those copy-prefix
frequencies must equal the checked sum of distance frequencies or the cost is
an output-size overflow. The distance-one prefix is `vp8l_prefix(121, 40)`;
its fixed extra width is multiplied by copy count. Ordinary symbol widths
already include the length and distance prefix symbols exactly once.

Let `P` be the checked sum of the 44-bit fast prefix, the five spatial-control
bits, the complete nested-map cost, every main table header, and all main
symbol/copy-extra payload bits. Then:

- `payload_bits = P`;
- `payload_bytes = checked_add(P, 7) / 8`;
- `padded_payload_bytes = payload_bytes + (payload_bytes & 1)`;
- the RIFF size field plus header is `12 + padded_payload_bytes` and must fit
  `u32` under the existing invariant;
- `riff_bytes = 8 + 12 + padded_payload_bytes`;
- `group_count = parts.frequencies.len()`.

Final zero-bit byte rounding and even VP8L chunk padding therefore remain
identical to the real writer.

## Ownership, allocation, errors, and module responsibility

The adaptive-table owner exposes the narrow rank-sum cost next to the real
balanced-table invariant. Exact spatial cost owns complete metric arithmetic,
nested-map construction/tokenization, candidate exact costing, and the final
writable plan. Profile orchestration owns candidate kind, candidate
`PlanParts`, A/O/B selection, tie rank, LowLatency growth, strict stop/fallback,
and final materialization. Clustering and refinement remain independent of
costing and writing. Production modules over 500 lines will be reviewed for a
cohesive split; new test modules live in descriptive sibling files.

B removes candidate `BitWriter`, adaptive-table, canonical-code, and retained
table-vector allocation. It adds no allocation, dependency, feature, public
API, unsafe block, thread, classifier, threshold, or parameter search. It
preserves allocations and error mapping required by map RGBA construction,
tokenization, final materialization, and output. Removing non-output temporary
allocations may let B proceed where A or O would fail allocation, but it may
not turn invalid copy counts or arithmetic overflow into output. Public error
categories, no-partial-output behavior, `Default`, metadata, animation,
dimensions, and limits remain unchanged.

## Locked manifests and exactness denominators

The locked CLIC 1.0.0 validation corpus has 102 rows and manifest SHA-256
`9feb09f469753c43864011aa6f00cfc5ee1bd48da5aac8f9b16d105890e14f86`.
The recovery/product screen is its first 41 rows and has manifest SHA-256
`474587feabe3178268b2eab6f7a166501d8ecc3d637a76bd412f4233dfa7b913`.
Expected Compact and LowLatency totals are 599,398,064 and 601,400,998 bytes.

Before B exists, A must reproduce those totals and P18 stream size+hash
identity 204/204. The locked non-timed 102-image A/O/B/public/P18 audit then
requires 204/204 identity, Compact E/B/R metric and planner/writer 306/306 with
growth 0/0, and LowLatency E/B/R plus every attempted Split metric 642/642,
planner/writer 408/408, and growth 336/336. Every selector, final rebuild,
fallback, and public error category must match.

Per profile and stage `E`, `B`, `R`, `Growth`, and `FinalMaterialization`, the
audit records candidate histogram evaluations; O adaptive builds (expected
aggregate 35,115 Compact and 43,905 LowLatency); B candidate adaptive and
canonical builds, both exactly zero; B rank-sum/table-cost heap allocations,
exactly zero; selection operations and scanned counters; unchanged nested-map
tokenizations and their allocations; final materializations; and conservative
estimated maximum live storage. The storage estimates model requested element
capacity and exclude allocator bookkeeping; they are not direct process-heap
measurements. `PlanParts` storage and nested-map RGBA/token allocations are not
rank-sum allocations and are reported separately or explicitly excluded from
the narrow zero-allocation field. Any metric, selector, wire, error, or count
mismatch rejects before timing.

## Unit/property matrix

For every alphabet length 1 through 296, deterministic empty, one-symbol at
relevant positions, sparse, dense, all-tied, maximum-counter, skewed, and
deterministically randomized shapes compare B rank-sum payload, O real-table
weighted widths, and the real `BitWriter` table+payload bits. Cases cover `k`
around every power-of-two boundary, `m` zero/small/large, ties straddling the
selection boundary, all active green lengths, copy prefixes and extras,
distance copies, inconsistent-copy failures, and overflow-adjacent arithmetic.
Tests also prove scratch capacity, zero B rank-sum/table-cost heap allocation
and table materialization, complete metric equality for every candidate
stage/profile, selector equality, final bytes, fallback, and error-category
equality. They do not claim that candidate `PlanParts` or unchanged nested-map
RGBA/token construction is allocation-free.

## Recovery screen and hard stop

Recovery uses one final release test binary, preloaded inputs, exactly one
warmup, then exactly three retained interleaved forward/reverse/forward A/B
rounds. There is no valid rerun to seek a pass. It requires:

- LowLatency B aggregate encode improvement at least 5.0% and 0/41 per-image
  median regressions;
- Compact aggregate regression below 0.5%, with every per-image regression
  reported;
- A/B output identity 82/82 and empty stderr.

Any failure retains release A, prohibits every product phase, and requires a
committed negative report and reproducer.

## Product gates, only after recovery passes

After removing A production routing and fixing release B, lock the final
binary. Rerun Phase A from scratch on 102 images: fixed totals, P18 identity,
0/102 above control +2%, all exact denominators, Compact growth 0/0,
LowLatency growth 336/336, and zero B candidate table builds.

The same-final-binary 41-image screen against latest-main/E37 controls then
requires, independently for both profiles: at least 50% aggregate encode
improvement, 0/41 encode regressions, aggregate bytes no worse, 0 images above
control +2%, Rust and pinned-C decode regression below 1%, RSS increase below
both 64 MiB and 5%, and exact decode for all six layouts.

Only a passing screen permits the locked 102-image formal run: one warmup then
five retained F/R/F/R/F rounds. Each profile requires over 50% independent
median improvement, 0/102 per-image median regressions, and absolute candidate
median at most 7.1 s Compact and 6.9 s LowLatency.

Final gates require 102-image all-layout exact decode with project and pinned C
decoders; no-product/product `Default` byte identity 102/102; stable default
and relevant workspace all-target tests; Clippy with warnings denied; fmt;
rustdoc with warnings denied; doctests; dependency/API/unsafe/thread/module,
resource, rlib, and binary audits; and isolated-target replay of mechanism,
Phase A, screen, formal, correctness, identity, quality, and evidence checks.
Only the installed stable host target is used and toolchains are not modified.

Passing every gate recommends promotion without integrating `main`. Any hard
failure stops later phases and preserves complete negative evidence.
