# VP8L profile-hybrid product migration report

## Decision

Reject this product migration. The locked 41-image screen was a legitimate hard-gate failure: `FastDecodeLowLatency` improved aggregate encode time by 48.1898587657271%, short of the required 50%. Per the frozen design, the 102-image x5 formal phase and later product gates were not run.

`FastDecodeCompact` passed the complete screen. LowLatency passed every screen condition except the aggregate encode threshold. All samples, including flagged 3-MAD observations, remain in the external raw output and contributed to the medians.

## Provenance

- Task: `019f8a85-c530-79d2-af1f-2b54105574be`
- Root task: `019f8321-035e-7211-8f53-987e18891c8c`
- Branch: `codex/vp8l-profile-hybrid-product`
- Worktree: `/Users/lance/.codex/worktrees/5020/webp`
- Base, creation `HEAD`, local `main`, and merge-base: `66c15f11c0cd63a7e5ad80ffbe7553e6f68ec569`
- P18 read-only oracle: `/Users/lance/.codex/worktrees/7d78/webp` at `c04bed7bf044dc610081ff1de0e43a2a579258bb`
- Locked final product test binary SHA-256: `9aa8fa08fb2288335a98ff4a3d5f64a5a7922372f7c4c1b1212ed98b9b1a29f8`
- Corpus manifest SHA-256: `9feb09f469753c43864011aa6f00cfc5ee1bd48da5aac8f9b16d105890e14f86`
- Screen manifest SHA-256: `474587feabe3178268b2eab6f7a166501d8ecc3d637a76bd412f4233dfa7b913`

The implementation was manually reconstructed from the stated invariants on the required base. No P18 commit was merged, rebased, or cherry-picked. The P18 tree was used only as a read-only behavioral oracle.

## Product implementation

Both profiles share preparation/tokenization, exact per-block frequency ownership, exact integer cost representation, strict `SinglePlan` fallback, complete-RIFF selection, and selected-only writing. Compact owns only `u16` per-block counters, evaluates exact-symbol and bin-mass proposals plus exactly one reassignment, and has no capacity-growth storage. LowLatency begins with the same three-way exact selection and then applies deterministic capacity growth using its profile-owned self-cost arrays.

All new production modules are private and below 500 lines. The production diff adds no dependency, feature, public API, unsafe block, thread, image identifier, corpus threshold, timer, or runtime classifier. `Default` routing was not changed. The benchmark audit surface is private and test-only.

## Phase A: pass

The locked 102-image Phase A used the final product binary. Compact produced 599,398,064 bytes versus 617,958,802 control bytes (-3.0035558908%), with 0/102 images above control +2%; its worst image was +1.4905309333%. LowLatency produced 601,400,998 bytes versus 625,321,072 (-3.8252467526%), with 0/102 above +2%; its worst image was +1.4837447462%.

Compact selected B once and R 101 times, constructed zero growth-state rows, and recorded growth 0/0. LowLatency selected E once, B twice, R 20 times, and Split 79 times; growth was 336/336 and all 102 rows had the required growth state.

Exactness denominators were:

- Compact spatial planner/writer: 306/306
- LowLatency spatial planner/writer: 408/408
- Single planner/writer: 204/204
- E/B selectors: 204/204
- Final selectors: 204/204
- Public selected streams: 204/204
- Strict single fallbacks: 204/204
- P18 candidate byte identity: 204/204, or 102/102 per profile

Phase A stderr was zero bytes. Durable results are in `phase-a-summary.json`; raw output is external at `/private/tmp/vp8l-profile-hybrid-product-p20-phase-a-final-9ad4afbe`.

## Locked 41-image screen: fail

The runner preloaded inputs, performed warmup, then retained three forward/reverse/forward interleaved measurements from the same final binary.

| Gate | Compact | LowLatency |
| --- | ---: | ---: |
| Independent aggregate encode improvement | 50.481976% pass | 48.189859% **fail** |
| Candidate aggregate encode median | 2.488329334 s | 2.555451500 s |
| Control aggregate encode median | 5.025098208 s | 4.932338417 s |
| Per-image encode regressions | 0/41 | 0/41 |
| Aggregate byte delta | -3.482487% | -5.093032% |
| Images above control +2% | 0/41 | 0/41 |
| Rust decode delta | -0.769314% | -0.021578% |
| Pinned-C decode delta | -0.379723% | -1.018441% |
| Encode RSS delta | -320,487,424 B (-35.330985%) | -323,190,784 B (-35.605213%) |

Compact encode samples were 5.079713375/5.025098208/5.018523750 seconds for control and 2.485682625/2.488329334/2.503336667 seconds for product. LowLatency samples were 4.936958875/4.932338417/4.925644292 seconds for control and 2.551721500/2.555451500/2.657632333 seconds for product.

All six generated layouts for all 41 images decoded to exact RGBA: project decoder 246/246 and pinned libwebp 246/246. Benchmark stderr was zero bytes. The durable concise result is `screen-summary.json`; retained raw output is external at `/private/tmp/vp8l-profile-hybrid-product-p20-screen-filter-fixed`.

The first attempted screen was invalid because the shared runner used a stale ignored-test filter and therefore measured zero-test harness invocations. It is preserved and explained in `invalidated-runs/stale-screen-test-filter.md`; it is not used for any claim.

## Quality, size, and resources

Before corpus execution, `cargo test -p webp --lib` passed 293 tests with 4 ignored, `cargo clippy -p webp --all-targets -- -D warnings` passed, and `cargo fmt --check` passed (with only stable rustfmt's informational warning about a nightly-only setting). The focused product tests and both durable runners passed their self-checks. Full workspace/configuration tests, rustdoc, doctests, the 102-image all-layout final correctness sweep, Default 102/102 identity, and the repository-root full replay were intentionally not run after the hard screen failure.

An identical stable release build at the base and product revisions measured the production `rlib` at 2,391,336 and 2,629,416 bytes respectively: +238,080 bytes (+9.95594%). The release unit-test/reproducer binary was 2,199,952 bytes at base and 2,284,320 bytes for the product: +84,368 bytes (+3.83499%). The latter product binary is the locked measurement binary.

At the maximum 16,384 x 16,384 lossless dimensions, the exact persistent block-counter storage is 34,373,632 bytes for Compact (`u16`, 16,384 blocks) and 17,186,816 bytes for LowLatency (`u32`, 4,096 blocks). LowLatency additionally owns 65,536 bytes of persistent self-cost arrays, for 17,252,352 bytes total; Compact constructs none of that growth state. These figures cover the new persistent exact block store, not a whole-process peak bound. The locked screen provides the measured process-level resource result: encode RSS decreased by 320,487,424 bytes (305.64 MiB) for Compact and 323,190,784 bytes (308.22 MiB) for LowLatency relative to their controls.

## Stopping point

The formal 102-image x5 phase was not run, so no formal medians or absolute-median claims exist. All later expensive correctness, identity, quality, and isolated-target replay gates were also stopped as required. The Phase A and screen runners remain as durable reproducers, while generated raw files, logs, run directories, and checksums remain outside the repository under the paths above.
