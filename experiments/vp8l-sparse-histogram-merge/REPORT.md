# P21 sparse exact-histogram merge recovery: rejected

## Identity and decision

- Task: `P21 sparse exact-histogram merge recovery`
- Root task: `019f8321-035e-7211-8f53-987e18891c8c`
- Branch: `codex/vp8l-sparse-histogram-merge`
- Base: `8485fc0593bf6e29715350ea72b15a9dabf4c80b`
- Worktree: `/Users/lance/.codex/worktrees/1841/webp`
- Measurement HEAD: `a07b3d21aaf240bb38e31b7265fca4a1681ede7d`
- Design: `c57e7eac0da997fa0653975e60c59f50821d5a05`
- Authorized P20 transplant: `1746c7bd69f85d4719f402983b2d7e9561fada53`
- Dense-A evidence: `52ccccad767247e8b3d4a654283ae3fc808606ed`
- Test-only oracle/mechanism: `60dc7c99e5d8175edda05358a2cfb6de8c664147`
- Locked runner: `a07b3d21aaf240bb38e31b7265fca4a1681ede7d`
- Corpus manifest SHA-256:
  `9feb09f469753c43864011aa6f00cfc5ee1bd48da5aac8f9b16d105890e14f86`
- Screen manifest SHA-256:
  `474587feabe3178268b2eab6f7a166501d8ecc3d637a76bd412f4233dfa7b913`
- Locked P21 binary SHA-256:
  `5851733d03d4a2670b8490d57ac99751ae0bcdf0d341ca6d4ee54aa4c910bb4f`
- Rebuilt P18 binary SHA-256:
  `2e5f7b11b959de3cb25a251649ee7ffa87528b0346d92fdfbb03547da5f5e570`

Decision: **reject zero-eliding exact-histogram merge**. The mechanism and all
byte/exactness gates passed, but the single valid recovery screen failed both
LowLatency timing gates. Product Phase A, product screen, formal, and final
product validation were therefore prohibited and were not run. No release
route uses B; the sparse variant and counters remain private `cfg(test)` code.

## Dense A control

Before B existed, the selectively transplanted P20 production implementation
was built on P21 and run over the locked 102 images. Compact totaled
599,398,064 bytes and LowLatency 601,400,998 bytes. Per-image size plus stream
hash matched the rebuilt P18 oracle for 204/204 profile streams; both stderr
files were empty. The control binary SHA-256 was
`b1913994afd53b3cb1547b63b0778c5baed34ca87bc4cebd9c57d95c72b7c8ce`.

The first file-by-file comparison named a replay directory that did not retain
generated streams. It is preserved as an invalidated path-resolution attempt,
not counted as a mismatch or gate sample.

## Mechanism, census, and exactness

The A/B unit/property matrix passed for empty, sparse, dense, maximum and
overflow-adjacent histograms, literal/copy inputs, `u16` Compact counters,
`u32` LowLatency counters, identical partial state/errors, and deterministic
plans. The focused locked-binary mechanism run passed six tests. The broader
library run before measurement passed 296 tests with five expected ignores.

Across both profiles, P21 visited 105,647,937 source slots: 29,273,215 were
nonzero additions and 76,374,722 were zero slots, a 72.291731% theoretical
elision ratio. Compact visited 50,676,141 slots and could skip 37,966,665
(74.920198%). LowLatency visited 54,971,796 and could skip 38,408,057
(69.868660%). LowLatency growth dominated with 37,281,460 visits, 26,545,501
zero slots, and a 71.202954% elision ratio. Full per-stage counts are in
`phase-r-summary.json`.

Despite that sparsity, A, B, the public candidate, and P18 matched for 204/204
profile streams. Compact/LowLatency aggregates were exactly 599,398,064 and
601,400,998 bytes. Compact spatial planner/writer exactness was 306/306 and
LowLatency 408/408; E/B selector, final selector, public selection, strict
fallback, A/B, and P18 denominators were each 204/204. Compact growth/state was
0/0 and 0 rows; LowLatency was 336/336 with 102 state rows. All stderr was
empty.

## Locked recovery screen

One final test binary preloaded the first 41 images, ran one unscored warmup,
then retained three forward/reverse/forward interleaved A/B rounds. No sample
was deleted and the valid screen was not repeated. All 82 profile/image output
pairs were byte-identical and all stderr files were empty.

Compact A samples were 2.519535500, 2.495787042, and 2.504255333 seconds; B
samples were 2.494158583, 2.512764792, and 2.505111542 seconds. The independent
median delta was **+0.034190%** (B slower), within Compact's maximum +1% gate.
Its 20 reported per-image median regressions were:

`000 +0.385823%`, `003 +0.682654%`, `004 +0.606951%`,
`005 +1.616969%`, `006 +0.882117%`, `007 +0.122689%`,
`009 +0.286374%`, `010 +0.475166%`, `020 +0.011433%`,
`021 +0.520313%`, `024 +0.273082%`, `025 +1.065077%`,
`029 +1.154691%`, `033 +0.200812%`, `034 +0.197393%`,
`035 +0.692777%`, `036 +0.052111%`, `038 +0.986439%`,
`039 +0.610478%`, and `040 +0.359833%`.

LowLatency A samples were 2.570629709, 2.682748042, and 2.580749792 seconds;
B samples were 2.576377042, 2.591843125, and 2.653544791 seconds. The
independent median delta was **+0.429849%**: B was slower, equivalent to
-0.429849% improvement versus the required at least +3.0%. B also had 23/41
per-image median regressions versus the required 0/41. Both LowLatency gates
failed, so the recovery hypothesis did not explain P20's product-screen loss.

## Stop, quality, resources, and reproducibility

The predeclared stop rule was applied immediately. There is no product B-only
binary, product Phase A, latest-main/E37 product screen, 102x5 formal,
six-layout final correctness, Default identity, full workspace quality,
resource/size audit, or isolated product replay. None is claimed as passing.
The production path remains dense A and contains no timer or counter; test-only
census was disabled in all timed measurements.

Raw TSV, stderr, logs, target outputs, binaries, and the 85-entry checksum
manifest remain external at
`/private/tmp/vp8l-sparse-histogram-merge-p21-phase-r-a07b3d21`.
`SHA256SUMS` hashes to
`241dc328bbc272a6dc48bc3f77bb793cab9341fa0722e77b228ea20e59986e8a`
and verifies cleanly. Durable design, runner, summarizer, concise summaries,
provenance, the invalidated path note, and this negative report remain on the
explicit P21 branch. Recommendation: reject and do not integrate the P20
product implementation or the P21 experimental oracle into `main`.
