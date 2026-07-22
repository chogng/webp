# P25 entropy-map frequency-sink recovery result

P25 rejects the fused candidate frequency sink for product migration. Release
remains A. The only permitted recovery screen completed once and failed the
LowLatency hard gate: 2.156889% aggregate improvement is below 5.0%, with
4/41 per-image median regressions. Compact improved 2.166414% and therefore
met its no-worse-than-0.5% aggregate gate, despite 3 reported per-image
regressions; it cannot compensate for the LowLatency failure.

## Identity

- Root task: `019f8321-035e-7211-8f53-987e18891c8c`; task: P25.
- Branch: `codex/vp8l-entropy-map-frequency-sink`.
- Base: `acfe6caf9fb62468dc384790b3e2eecfe837f173`.
- Worktree: `/Users/lance/.codex/worktrees/8312/webp`.
- Binary source HEAD: `5cc3f96bdc4d619806f936971c001c99130ae0f8`.
- Recovery runner HEAD: `1ba754834e8a8c592c707551758e54c97dab783a`.
- Reused P25 binary SHA-256:
  `2d00a2699eaff1bd8e542b0e11987fa28e51a2d91662e78874cac989b2296276`.
- P18 binary SHA-256:
  `2e5f7b11b959de3cb25a251649ee7ffa87528b0346d92fdfbb03547da5f5e570`.

The committed chain is design `3ec3cccc`, P20 A transplant `f65ce142`, P24
O transplant `f20bbe0e`, B mechanism `71b3d3d2`, Phase-R/harness commits
through `055b90e1`, and recovery runner `1ba75483`. The final report commit
records the final P25 HEAD.

## Exactness and census

The complete non-timed audit retained externally at
`/private/tmp/vp8l-entropy-map-frequency-sink-p25-phase-r-manual-5cc3f96b`
passed A/O/B/P18 stream size/hash identity 204/204 with Compact/LowLatency
totals 599,398,064 / 601,400,998, empty stderr, unique 102-image census
rows, E/B/R candidate counts 102 each, Compact growth 0, LowLatency growth
336, 102 final materializations/profile, and strict fallback throughout.
B generic nested-map tokenization/allocation scope, adaptive and canonical
table builds, and rank-cost heap allocations were zero; O generic map
tokenizations and B direct map evaluations were 306/642. Its SHA256SUMS
digest is `7fef182cab47815aef85354a96f88fcfe7cae4711d8863e8a9d414fb9c5f618d`.

## Sole recovery screen

Recovery evidence is external at
`/private/tmp/vp8l-entropy-map-frequency-sink-p25-recovery-1ba75483` with
SHA256SUMS digest
`c12a0c6fb3bfa0697697a247c16f0c86e9c9327e1c08c675c27afc838bce7b61`.
Its listed files verify with `shasum -a 256 -c SHA256SUMS`.

One unscored warmup was retained for each A/B/profile, followed by the only
F/R/F A/B screen. Every scored layout has three 41-image aggregate samples;
all 82 A/B outputs are byte-identical and stderr is empty.

| Profile | A samples (ns) | B samples (ns) | Median improvement | Regressions | Gate |
| --- | --- | --- | ---: | ---: | --- |
| Compact | 2181936379, 2206971042, 2211407299 | 2159158916, 2233868331, 2152730630 | 2.166414% | 3/41 | pass |
| LowLatency | 2251563164, 2289176044, 2268366832 | 2219440667, 2229327587, 2216618790 | 2.156889% | 4/41 | **fail** |

LowLatency regression image IDs and median deltas are 031 (+0.131603%), 033
(+0.411801%), 035 (+0.092272%), and 038 (+1.581251%). Compact regressions are
003 (+3.364113%), 004 (+15.019642%), and 005 (+0.650049%).

The negative result prohibits P25 product Phase A, screen, formal, or any
main integration. A future idea requires a new latest-main research task;
this branch is evidence-only and must not be merged or rebased.

## Reproduction

`run_phase_r.sh` and `run_phase_r_census_shards.sh` reproduce the non-timed
audit using the preserved binary. `run_recovery.sh` is a state-guarded,
single-use recovery reproducer; its guarded outputs must not be overwritten
or rerun. Raw TSVs, logs, shard inputs, binaries, and SHA256SUMS remain
outside Git per `experiments/README.md`.
