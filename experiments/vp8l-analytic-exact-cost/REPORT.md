# P23 analytic exact-cost planning recovery: rejected

## Identity and decision

- Task: `P23 independent VP8L analytic exact-cost / selected-only
  materialization experiment`
- Root task: `019f8321-035e-7211-8f53-987e18891c8c`
- Branch: `codex/vp8l-analytic-exact-cost`
- Base: `76c9aa39e35534b847f2cb980cb0037c4e6be785`
- Worktree: `/Users/lance/.codex/worktrees/fc25/webp`
- Design: `6690b960a2582452b13729916ccb9e52f34bbaa0`
- Authorized P20 production transplant:
  `08c7b7c63d862b64f7dfe00e654a930c6dccf307` from
  `67bd04274a60fe81fcd13cc2702d75e3fd0553a4`
- Pre-B baseline runner/evidence: `77ce4947` / `dfc3c7eb`
- Analytic mechanism: `233a5eeea170bff08b620143e4504a46ece67428`
- Locked audit/recovery runner and measurement HEAD:
  `65d56bb84d197e3cb02b96fc99d73aeb396acc60`
- Census-only summarizer correction: `b610ae8b52fa30769415c0f0fc69f238ea3237aa`
- Locked P23 binary SHA-256:
  `215d5bc969c1be7bb5caa24e2ae378cd4fb73cb4c3f82b16c529371642e7c125`
- Rebuilt P18 binary SHA-256:
  `2e5f7b11b959de3cb25a251649ee7ffa87528b0346d92fdfbb03547da5f5e570`
- Corpus manifest SHA-256:
  `9feb09f469753c43864011aa6f00cfc5ee1bd48da5aac8f9b16d105890e14f86`
- Screen manifest SHA-256:
  `474587feabe3178268b2eab6f7a166501d8ecc3d637a76bd412f4233dfa7b913`

Decision: **reject analytic exact-cost planning as this recovery experiment**.
The complete mechanism, candidate metric, selection, fallback, and wire gates
passed, but the only valid recovery screen failed both LowLatency timing gates:
B improved only 0.818095%, below the required 5.0%, and had 3/41 per-image
median regressions instead of 0/41. Release remains the exact retained-plan A
route. Product Phase A, product screen, formal, and all final product phases
are prohibited and were not run.

## Frozen formula and implementation

The current adaptive builder always writes the normal form. The centralized
header count is exactly `1 + 4 + 19*3 + 1 + 4*N = 63 + 4*N` bits for an
alphabet of length `N`. Empty and one-symbol inputs still use this form; there
is no adaptive simple or alternate path.

For the nested group map, B constructs the same `[0, group, 0, 0]` RGBA
sequence, calls the same `collect_entropy_tokens` configuration, builds the
same five adaptive canonical tables, and counts its no-cache bit, table
headers, symbol widths, length extras, and fixed distance-one extras. For each
main group B builds the same five tables, counts `sum(frequency * code_width)`
plus copy extras, and drops the tables before the next group. It allocates no
candidate `BitWriter`, prefix vector, or retained cross-group table vector.

With checked arithmetic, `payload_bytes=(payload_bits+7)/8`, the VP8L payload
is padded to even bytes, the RIFF size is `12+padded_payload_bytes` and must fit
`u32`, and complete bytes are `riff_size+8`. Only a final spatial winner builds
one existing writable `SpatialCostPlan`; a single winner builds none. Candidate
records own only kind, `PlanParts`, and payload bits/bytes, RIFF bytes, and
group count. A/B dispatch and census remain private `cfg(test)` and outside hot
loops; release is fixed A.

## Pre-B A control

Before B existed, A produced exactly 599,398,064 Compact bytes and 601,400,998
LowLatency bytes. All 204 profile streams matched the rebuilt P18 oracle in
both size and stream hash, and stderr was empty. The A baseline binary SHA-256
was `3fb7dedded7d5f19fed1f57e61852ed85552b5bab9e1de1c6c752acd944a46be`.

## Mechanism, exactness, and census

Unit/property coverage passed for every legal adaptive alphabet length through
296 and empty, one-symbol, sparse, dense, tied, and maximum constructible
frequency shapes. Analytic and real tables had identical canonical code/width
entries. Header plus payload parity covered copy lengths through 4096 and
overflow-adjacent header, inconsistent-copy, and complete-metric arithmetic.
Both profiles passed E/B/R/growth metric equality, final rebuild byte identity,
selector/tie equality, strict fallback, and standard VP8L decode tests.

The locked 102-image audit proved A/B, public/A, and P18 identity 204/204 each;
E/B selector, final selector, and strict fallback were also 204/204. Compact
was 306/306 initial candidate metrics and planner/writer rows with growth 0/0.
LowLatency was 642/642 candidate metrics: 306 E/B/R plus every 336/336 attempted
and accepted Split; its declared planner/writer denominator was 408/408.
Final spatial materialization was 102/102 in each profile. Aggregate bytes and
winner distributions exactly matched P20/P18: Compact B=1/R=101; LowLatency
E=1/B=2/R=20/Split=79.

Compact's 306 candidates built 35,115 adaptive tables and tokenized 306 nested
maps per variant. A wrote 32,977,951 candidate prefix/header bits in 4,122,379
bytes, owned 306 logical candidate writer allocations, and retained 6,717 main
table entries. B had zero candidate writer allocations and zero retained table
entries while executing 16,190,463 explicitly counted checked bit operations.
The conservative maximum live estimated heap was 1,935,948 B for A versus
674,256 B for B, excluding allocator overhead.

LowLatency's 642 candidates built 43,905 adaptive tables and tokenized 642
nested maps per variant. A wrote 41,103,608 candidate prefix/header bits in
5,138,235 bytes, owned 642 logical candidate writer allocations, and retained
8,139 main table entries. B again had zero candidate writer allocations and
zero retained table entries while executing 20,245,341 checked bit operations.
The conservative maximum live estimate was 660,132 B for A versus 254,760 B
for B. Full per-stage census is in `phase-r-summary.json`.

## Single valid recovery screen

Host coordination paused P23 after its audit and before any warmup while A18
was active. Exactly zero timing files existed at that point. After the root
task explicitly sealed A18, P23 used the unchanged binary SHA above for exactly
one warmup and the single retained F/R/F screen. No timing sample was rerun or
discarded. The interrupted non-timed P18 identity artifact and coordination
sequence are preserved under `invalidated-runs`.

Compact A samples were 2.500603458, 2.491419209, and 2.482786000 seconds; B
samples were 2.480672167, 2.490026542, and 2.475931916 seconds. Independent
medians were 2.491419209 and 2.480672167 seconds, so B improved **0.431362%**
and passed the aggregate `<0.5% regression` gate. Its 11 reported per-image
regressions were: `000 +0.316902%`, `002 +0.155116%`, `003 +0.567291%`,
`009 +0.890030%`, `017 +0.494428%`, `028 +0.109106%`, `034 +1.055476%`,
`035 +0.356874%`, `037 +0.322671%`, `039 +1.000555%`, and
`040 +1.880091%`.

LowLatency A samples were 2.561057542, 2.554653416, and 2.550112334 seconds; B
samples were 2.624614042, 2.533753916, and 2.472134292 seconds. Independent
medians were 2.554653416 and 2.533753916 seconds, so B improved only
**0.818095%**, failing the required at least 5.0%. It also failed the required
0/41 regression gate with `024 +1.301130%`, `025 +0.931516%`, and
`026 +0.016810%`.

All 82 A/B screen stream pairs were byte-identical. Every audit, identity,
warmup, and retained-sample stderr file was empty. All samples and the large
first LowLatency B sample remain in external evidence; there was no outlier
filter or valid rerun.

The result rejects the declared causal hypothesis: deleting candidate prefix
serialization and retained table vectors is real and exact, but it does not
recover the missing LowLatency margin. The measured remainder is consistent
with tokenization, adaptive table construction, and checked analytic payload
work dominating the candidate cost, while B still pays one selected-plan
rebuild.

## Stop, branch integrity, and external evidence

The recovery stop applied immediately. There is no B-only release binary,
product Phase A, latest-main/E37 product screen, formal 102x5, all-layout final
correctness, Default identity, full product quality/resource/size audit, or
isolated product replay. None is claimed as passing.

Before measurement and again after the recovery stop, the branch passed the
full webp library suite (300 passed, 4 expected ignores), all-target Clippy
with warnings denied, formatting, and runner/summarizer syntax checks; the
focused mechanism matrix passed before measurement. These are branch-integrity
checks, not the prohibited final product gate. Static routing inspection
confirms analytic B, dispatch, and census remain test-only; production release
remains A and adds no dependency, feature, public API, unsafe block, thread,
classifier, metadata, animation, Default, or limit behavior.

At finalization, local `main` had advanced separately to
`62d8afea6c23a93aec9393b4d746b6dd69d76305`. P23 did not merge or rebase it:
the branch merge-base remains the declared
`76c9aa39e35534b847f2cb980cb0037c4e6be785`, which is an ancestor of P23, and
P23 is not an ancestor of the declared base.

Raw TSV, stderr, logs, targets, binaries, and the 31-entry checksum manifest
remain external under
`/private/tmp/vp8l-analytic-exact-cost-p23-phase-r-65d56bb8`.
`SHA256SUMS` hashes to
`ae4d1ac002e339920467a1556c5f66572eb028cb2d4c4b9d1cac92851ce88fc6`.
Recommendation: reject and do not integrate either the transplanted P20 A
control or P23 analytic oracle into `main`.
