# VP8L packed-token writer product validation

Date: 2026-07-22 (America/Los_Angeles).

## Decision

**Pass every product gate and recommend the packed spatial writer for
integration review.** The product implementation is byte-identical to the
required latest-main base and to E36, while the final same-binary 102-image,
five-round A/B is 27.01% faster for `FastDecodeCompact` and 26.56% faster for
`FastDecodeLowLatency`. Both candidate medians are below 8.5 seconds and no
image has a positive median regression.

| profile | latest-main writer control | product | independent | paired median | regressions | gate |
| --- | ---: | ---: | ---: | ---: | ---: | --- |
| Compact | 10.787120 s | **7.874026 s** | **-27.005%** | -26.828% | 0/102 | pass |
| LowLatency | 10.401583 s | **7.638855 s** | **-26.561%** | -26.249% | 0/102 | pass |

The branch is intentionally not merged into `main`.

## Provenance and branch isolation

- Source task/thread: `019f8321-035e-7211-8f53-987e18891c8c`.
- Task: P12 / E37, `vp8l-packed-writer-product`.
- Worktree: `/Users/lance/.codex/worktrees/5e00/webp`.
- Branch: `codex/vp8l-packed-writer-product`.
- Required initial local-main base:
  `0ee428dc0bee9c035f051b4ccaa846dabe394ca8`.
- Before branch creation, detached `HEAD`, local `main`, and
  `merge-base(HEAD, main)` all resolved exactly to that full SHA.
- Code commit: `9435fbd0499fe76ad5579a740da518f49ebd67d0`.
- Code parent: exactly the required base.
- Local `main` later advanced independently to
  `5e6b549abd5b9e7ad4f0b89ceda81da8a8e97e3a`; the branch was not rebased and
  its final merge-base with local `main` remains exactly the required
  `0ee428dc...` base.
- Evidence/report commit: recorded in the final handoff because a commit
  cannot self-reference its own SHA.
- Final release test binary SHA-256:
  `247305b53187841383afb7a39a872f1292728e7a114b0d5541547b101da524fe`.

The final product, latest-main, and E36 binaries were each built from a full
`git archive` of the exact commit and from that archive's own `webp-rs/`
workspace. Blob checks matched the requested commit before use. `origin/main`,
old-worktree ancestry, and cherry-picks were not used. The root worktree's
user-owned `AGENTS.md` change and untracked `docs/final-rust-architecture.md`
were not touched, staged, or committed.

## Product implementation

The migration was reconstructed by hand from the mechanism, not copied as an
E36 commit. `spatial_writer.rs` remains the orchestration owner: it builds the
spatial plan and Huffman tables, chooses each token's group, advances pixels,
and wraps the finished VP8L payload. A new private 205-line
`spatial_packet_writer.rs` owns a narrower invariant:

- a private `TokenPacket { bits: u64, width: u8 }` representing one complete
  literal, copy, or cache token in VP8L LSB-first wire order;
- canonical-code reversal and ordered symbol/extra-bit concatenation;
- a safe sink owning prefix bytes, a `u64` accumulator, and the count of
  pending bits;
- low 32-bit little-endian bulk flushes via a `u128` intermediate, leaving
  fewer than 32 bits pending after every append;
- checked reserve arithmetic, fallible `try_reserve`, checked end arithmetic,
  explicit capacity rejection, and zero-padded final tail emission.

One preflight reserves `tokens * 8 + 1` additional bytes. Multiplication and
tail addition are checked; eight bytes cover a legal packet up to 64 bits and
one byte covers the initial partial-byte offset. Append and finish never rely
on an unchecked growth path. There is no `unsafe`, new dependency, thread,
concurrency, or broad dead-code allowance.

The production path changed only the spatial candidate main writer used by
`FastDecodeCompact` and `FastDecodeLowLatency`. Clustering, tokenization,
profile selection, exact single-stream selection, Default/API behavior,
metadata, animation, wire syntax, error variants, and the thread model are
unchanged. The same-binary latest-main writer control is reconstructed only
inside the existing `cfg(test)` product benchmark module; production modules
contain no packet census, phase variant, layout switch, audit hook, or runner.

## Legal-width and error-boundary proof

VP8L canonical codes are bounded at 15 bits. The private packet can therefore
hold every legal token:

- literal: green + red + blue + alpha = `4 * 15 = 60` bits;
- general copy: green length code `15` + length extra `10` + distance code
  `15` + distance extra `18` = **58 bits**;
- cache: one green-alphabet code = **15 bits**.

The spatial product uses distance 121, so a synthetic maximum-length spatial
copy under 15-bit tables is 45 bits; the independent general-copy boundary
test proves the full 58-bit format limit. Tests cover all starting offsets,
widths 0 through 64, non-byte-aligned prefixes, repeated 32/64-bit crossings,
bulk flushes, partial tails and zero padding, literal/copy/cache ordering,
15-bit table extremes, maximum length 4096, the 58/60-bit legal bounds,
reserve multiplication overflow, invalid prefix state, insufficient capacity,
packet overflow, missing/oversized table entries, and cache-index overflow.

The focused packet suite has seven tests in the required named sibling file
`spatial_packet_writer_tests.rs`. Full debug and release workspace suites also
pass. `spatial_packet_writer.rs` is 205 production lines and
`spatial_writer.rs` is 328 lines, both below the 500-line target.

## Final locked performance gates

The final binary held `/private/tmp/webp-vp8l-product-benchmark.lock`
atomically. Inputs were preloaded, all output bytes were checksummed, odd
rounds used forward layout order, even rounds used reverse order, and `wait4`
captured process wall time, user+system CPU, and peak RSS. The corpus manifest
contains 102 inputs and has SHA-256
`9feb09f469753c43864011aa6f00cfc5ee1bd48da5aac8f9b16d105890e14f86`.
The screen manifest is exactly its sorted first 41 rows and has SHA-256
`474587feabe3178268b2eab6f7a166501d8ecc3d637a76bd412f4233dfa7b913`.

The final 41-image, three-round screen passed before the formal run:

| profile | control median | product median | change | per-image range | regressions |
| --- | ---: | ---: | ---: | ---: | ---: |
| Compact | 4.503202 s | 3.347753 s | -25.658% | -31.409% to -24.420% | 0/41 |
| LowLatency | 4.390787 s | 3.264446 s | -25.652% | -31.352% to -25.011% | 0/41 |

Final 102-image, five-round aggregates retain every sample:

| profile/layout | five rounds (s) | median / MAD |
| --- | --- | ---: |
| Compact control | 10.736630, 10.787120, 10.875502, 10.761044, 10.830644 | 10.787120 / 0.043524 |
| Compact product | 7.897791, 7.859702, 7.860203, 7.874026, **7.929905** | 7.874026 / 0.014324 |
| LowLatency control | 10.498066, 10.313119, 10.408109, 10.401583, 10.389869 | 10.401583 / 0.011714 |
| LowLatency product | 7.638855, 7.605984, 7.689489, 7.691133, 7.612804 | 7.638855 / 0.032871 |

Compact round 5 is a retained 3xMAD outlier. No LowLatency round is a 3xMAD
outlier. Compact paired changes are -26.441%, -27.138%, -27.726%, -26.828%,
and -26.783%; LowLatency paired changes are -27.236%, -26.249%, -26.120%,
-26.058%, and -26.729%. No sample was removed.

Per-image p0/p10/p50/p90/p100 changes are
-32.754/-31.827/-28.885/-26.477/-24.325% for Compact and
-33.295/-31.651/-28.582/-26.193/-24.560% for LowLatency. Both profiles have
0/102 positive per-image median regressions. Across warmups and formal rounds,
1,224 product/control per-image length+hash pairs have zero mismatches.

## Process resources and artifact size

Final formal medians:

| profile/layout | process wall | CPU | max RSS |
| --- | ---: | ---: | ---: |
| Compact control | 16.046536 s | 16.000232 s | 1215.27 MiB |
| Compact product | 13.139456 s | 13.085052 s | 1143.25 MiB |
| LowLatency control | 15.719175 s | 15.628885 s | 1215.53 MiB |
| LowLatency product | 12.922736 s | 12.876665 s | 1153.55 MiB |

Product peak RSS is 72.02 MiB lower for Compact and 61.98 MiB lower for
LowLatency. The final release rlib is 453,848 bytes versus 436,344 bytes at
the base: +17,504 bytes (+4.011%). The final release test binary is 1,501,056
bytes versus 1,481,328 bytes: +19,728 bytes (+1.332%). The test binary includes
the test-only same-binary control. Full artifact hashes are in
`binary-artifacts.tsv`.

## Wire identity and decoder correctness

Independent full-archive release test binaries were used for both comparisons.
The verifier streams one image at a time and deletes generated files after
each comparison.

Latest-main `0ee428dc...` versus product `9435fbd0...`:

- Default/Compact/LowLatency length identity: 306/306;
- SHA-256 identity: 306/306;
- full byte identity: 306/306;
- latest-main project-decoder RGBA exact: 306/306;
- product project-decoder RGBA exact: 306/306;
- product pinned-libwebp `WebPDecodeRGBA` exact: 306/306, failed=0.

Product versus E36 candidate `dfc0cf6f...`:

- Default/Compact/LowLatency length identity: 306/306;
- SHA-256 identity: 306/306;
- full byte identity: 306/306;
- product and E36 project-decoder RGBA exact: 306/306 each;
- additional E36 pinned-libwebp exact: 306/306, failed=0.

Pinned libwebp is exact commit
`733c91e461c18cf1127c9ed0a80dccbcfed599d3`; the local comparison binary has
SHA-256 `f7396d503fb343bd57d3848d0ce04b87f3485322fce6b0b912dda45ecf5ecb6e`.

## Stable quality gates

The complete product archive passed with stable Rust 1.97.1 on host
`aarch64-apple-darwin`:

- `cargo test --workspace --all-targets`;
- `cargo test --release --workspace --all-targets`;
- `cargo clippy --workspace --all-targets -- -D warnings`;
- `cargo fmt --all -- --check`;
- `RUSTDOCFLAGS='-D warnings' cargo doc -p webp --no-deps`;
- `cargo test -p webp --doc`.

No `--target` was supplied; only the installed host target was used. No target,
component, toolchain, dependency, or global configuration was installed or
changed. Raw logs and the command/status table are under `raw/validation/`.

## Evidence discipline and invalidated runs

Only the final full-archive binary `247305b5...` supplies headline screen,
formal, and product correctness results. The audit trail is deliberately
retained:

1. Three intended archive builds changed only `CARGO_TARGET_DIR` while still
   compiling from the current worktree. All three incorrectly produced the
   identical test binary `c67f50a8...`. Benchmarking had not begun; every hash
   and product from these builds is invalid.
2. A screen preparation shell used zsh's special `path` variable, removing
   command lookup. No lock or timing sample existed; this is a non-run.
3. A pre-manifest screen began before the new manifest-order instruction and
   was terminated. Its 17 warmup/round-one files remain under
   `raw/invalidated-screen-pre-manifest/` and are not summarized.
4. Exact workspace-subtree archives produced binaries `04f3160e...`,
   `b45744af...`, and `bee353f3...`, but did not meet the stricter full-repo
   archive-path requirement. Their complete screen, formal, and identity data
   are preserved in explicitly named `raw/invalidated-*subtree*` directories
   and excluded from every headline.
5. All-target validation on the incomplete subtree archive failed because
   repository-root `tests/corpora/libwebp-test-data-smoke-v1.txt` was absent.
   A later relative `mv` from the full-archive workdir missed its target, and
   the original raw log was overwritten by the successful absolute-path retry.
   `invalidated-runs/INVALIDATED_VALIDATION.md` contains the complete captured
   output reconstructed from task tool output, the exact `mv` error, and the
   explicit statement that it is reconstructed rather than original raw.

`invalidated-runs/INVALIDATED_RUNS.md` is the authoritative inventory. Nothing
listed there is mixed into `gate-summary.json` or this decision.

## Evidence and reproduction

Primary evidence:

- `raw/screen-41-final/`: final 41x3 samples and process resources;
- `raw/formal-102-final/`: final 102x5 samples and process resources;
- `raw/identity-latest-main-product/`: 306-item base/product identity and
  product decoder/oracle checks;
- `raw/identity-product-e36/`: 306-item product/E36 identity checks;
- `raw/validation/`: final stable quality logs;
- `raw/corpus-manifest-102.tsv` and `raw/screen-manifest-41.tsv`;
- `gate-summary.json`, `binary-artifacts.tsv`, `provenance.txt`, and
  `SHA256SUMS`.

`reproduce.sh` rebuilds all three binaries from complete archives, verifies
manifests before acquiring the benchmark lock, reruns the final screen/formal
and both identity checks, enforces the product gates, and runs the stable
quality matrix.

## Recommendation

Recommend integration review of code commit `9435fbd0...`. The performance
gate has comfortable margin, output and error semantics are preserved, resource
usage improves, the implementation is safe and narrowly private, and all
correctness/quality gates pass. Do not merge automatically; review and merge
from `codex/vp8l-packed-writer-product` under the parent task's normal process.
