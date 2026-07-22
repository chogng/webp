# VP8L coarse spatial product validation

Date: 2026-07-21 (America/Los_Angeles).

## Decision

**Pass the product gate.** The two public opt-in profiles remain ordinary
VP8L, preserve the established default byte-for-byte, pass both decoders, and
meet the required same-profile size and decode thresholds.

| public profile | geometry | bytes | size vs fast-no-cache single | decode median | decode vs single | paired median | gate |
| --- | --- | ---: | ---: | ---: | ---: | ---: | --- |
| `FastDecodeCompact` | 128px / 64 groups | 617,958,802 | -9.229% | 4.023177 s | +2.051% | +1.506% | pass |
| `FastDecodeLowLatency` | 256px / 16 groups | 625,321,072 | -8.148% | 4.000413 s | +1.474% | +1.001% | pass |

The same-profile single baseline is 680,790,322 bytes and 3.942318 s. The
gate is size at most -8%, aggregate decode slowdown at most +5%, and exact
decode through both the project decoder and pinned libwebp.

## Identity and provenance

- Source task: `019f8321-035e-7211-8f53-987e18891c8c`.
- Base and initial full SHA:
  `52c6b8fc64cd86b4fccd0f30fb996d825a6dd2ec`.
- Branch: `codex/vp8l-coarse-spatial-product`.
- Worktree: `/Users/lance/.codex/worktrees/070b/webp`.
- Read-only candidate commit:
  `72409d7a41dc3cf9623c34ad7752f2dd9f1ac208`.
- Read-only source tree:
  `/Users/lance/.codex/worktrees/6d6b/webp`.
- Required HEAD was checked before branch creation and matched the base
  exactly. The product branch was created directly at that detached HEAD; no
  fetch, `origin/main`, cherry-pick, or old experimental branch was used.
- Product code commit:
  `fb8693837c5bf486a722df8f14f9cb5f8eb6691f`.
- Before the evidence commit, local `main` was observed at
  `f1ce6065cbbd11661561956c0d982e0a4cfddc27`. Its only change from the
  creation base is `webp-rs/vp8l/README.md` (commit
  `docs(vp8l): close entropy experiment batch`). Because the product code
  commit had already been created from the required base, the histories are
  docs-only siblings rather than a fast-forward chain. Per integration
  direction, this task does not merge, rebase, or rewrite them; the root task
  will migrate the code and evidence commits linearly onto its then-current
  `main` and independently reverify the result.
- `AGENTS.md` was not modified.

## Product API and semver surface

The stable surface adds:

- `LosslessEncodeProfile::{Default, FastDecodeCompact,
  FastDecodeLowLatency}`;
- `LosslessEncodeOptions { profile }`;
- `encode_lossless_rgba_with_options`;
- `encode_lossless_rgba_with_metadata_and_options`.

Both the enum and options struct are intentionally `#[non_exhaustive]`.
Downstream callers start from `LosslessEncodeOptions::default()` and change
the public `profile` field, leaving room for additive profiles or controls.
The rustdoc contains a compiling example of that pattern.

`Default` calls the established encoder path. The existing static,
metadata, and animation functions retain their signatures and semantics.
Default options are byte-for-byte identical to the old static entry points.
The metadata/options entry point preserves ICCP, EXIF, and XMP bytes and VP8X
flags. Animation intentionally has no new profile selector and its public docs
state that frames continue to use `Default`.

The `FastDecode*` names describe the retained tradeoffs relative to their
shared fast-no-cache single-group profile. They do **not** promise smaller or
faster output than public `Default`. They can be larger than `Default`, and
encoding is currently much more expensive because the implementation fully
serializes single and candidate files before selecting.

## Implementation and wire format

The production implementation is safe stable Rust, single-threaded, and adds
no dependency or unsafe block. It exposes no block/group tuning knobs,
experimental types, private format, or benchmark-only public API.

Responsibilities are directional and private:

- `spatial_cluster.rs` owns checked coarse histograms, deterministic seed
  ranking, nearest-seed assignment, empty-block fill, and dense group ids;
- `spatial_plan.rs` owns the two closed product geometries, block ownership,
  and per-group VP8L frequencies;
- `spatial_writer.rs` owns standard nested group-map, Huffman-table, and main
  stream serialization plus complete-RIFF selection.

All three production modules are below 500 lines. New tests are sibling files.
Each product encode validates once and tokenizes once. The single and coarse
streams are built from that same token vector. The candidate is selected only
when its complete padded RIFF file is strictly smaller; otherwise the exact
single byte vector is returned.

The streams use the standard VP8L subtract-green, no-predictor, no-cache
fast profile. The meta-Huffman image contains ordinary dense group ids, then
five ordinary Huffman tables per used group and the ordinary main token
stream. A copy token belongs to the block containing its starting output
pixel and may cross a coarse `run_end`; it is never split.

## Correctness

Corpus: 102 CLIC validation method-6 inputs, 251,858,137 pixels and
1,007,432,548 RGBA bytes per decode pass. Pinned libwebp revision:
`733c91e461c18cf1127c9ed0a80dccbcfed599d3`.

- Current generation produced public default, fast-no-cache single, Compact,
  and LowLatency streams: 408 files total.
- Project decoder compared complete width, height, and RGBA for every stream:
  408/408 exact.
- Pinned `WebPDecodeRGBA` used full-byte `memcmp`: 408/408 exact,
  `failed=0`.
- Fixed/synthetic tests cover tiny images, transparency, 127/128/129 and
  255/256/257 boundaries, coarse selection, exact fallback, metadata, and a
  299-pixel copy beginning at pixel 1 and crossing block boundaries.
- A clean archive of the required base and this tree independently encoded
  all 102 public-default images. Their full per-file RGBA SHA-256, output
  SHA-256, output length, and decoder status TSVs are byte-identical. Both TSV
  SHA-256 values are
  `994a4afabb52d94e65678ce15de57a09c35c279b53566cf94f26137201bd7b34`.
- Current independently generated single/Compact/LowLatency outputs match the
  read-only gated candidate on bytes, RGBA hash, and complete stream hash for
  306/306 streams. This byte identity carries the candidate's previously
  checked model/actual bit partition to the delivered streams without
  relabeling old timings as current measurements.

## Structural audit

| structural metric | Compact | LowLatency |
| --- | ---: | ---: |
| used groups | 3,110 | 1,452 |
| map cells | 16,103 | 4,216 |
| row runs | 1,997,970 | 1,007,545 |
| adjacent group-map switches repeated by pixel row | 1,273,398 | 642,086 |
| switches between consecutive token-start groups | 1,398,608 | 766,903 |

These are structure counts, not instrumented decoder dispatches or table
reselections. In particular, a copy token can cross `run_end` and skip a later
boundary. Decode impact is established by the locked wall-time measurements,
not inferred from these counts.

## Locked performance protocol

The release test binary is the same binary for all layouts in each run. The
runner atomically holds `/private/tmp/webp-vp8l-product-benchmark.lock`, writes
its PID, removes it on normal exit or signal, preloads inputs before internal
timing, alternates layout order forward/reverse, and records `wait4` process
wall, CPU, and max RSS. Encode hashes every output byte. Decode materializes
the full RGBA vector and hashes every RGBA byte. Warmups are separate; every
one of five formal rounds is retained.

The 41-image screen passed before the formal run:

| layout | decode median | vs single | paired median |
| --- | ---: | ---: | ---: |
| single | 1.670383 s | — | — |
| Compact | 1.696975 s | +1.592% | +1.794% |
| LowLatency | 1.682369 s | +0.718% | +0.266% |

### 102-image five-round gate

| operation/layout | median wall | MAD | vs single | paired median | median CPU | median max RSS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| decode single | 3.942318 s | 0.021666 s | — | — | 4.032580 s | 721.05 MiB |
| decode Compact | 4.023177 s | 0.011836 s | +2.051% | +1.506% | 4.105834 s | 661.95 MiB |
| decode LowLatency | 4.000413 s | 0.012302 s | +1.474% | +1.001% | 4.078200 s | 668.72 MiB |
| encode single | 6.430381 s | 0.006007 s | — | — | 11.263187 s | 1,582.25 MiB |
| encode Compact | 14.668471 s | 0.026747 s | +128.112% | +127.640% | 19.472431 s | 1,294.16 MiB |
| encode LowLatency | 14.253173 s | 0.037536 s | +121.654% | +121.277% | 19.013918 s | 1,293.19 MiB |

Wall 3×MAD flags were single decode r5, Compact decode r1, single encode
r1/r3, and LowLatency encode r4. Paired flags are retained in `summary.json`;
no sample is silently removed. The headline uses all five values. Median
without outliers remains below the decode gate for both profiles.

Per-image decode slowdown p0/p10/p50/p90/p100 is
-2.645/0.224/1.613/3.458/6.377% for Compact and
-3.278/-0.784/0.885/2.208/4.114% for LowLatency. The aggregate gate, not a
per-image cap, is the stated criterion.

Encoding remains the clear next optimization target. The product migration
does not include an unvalidated bit-cost-only shortcut; it retains the exact
full-file comparison required by the gate.

## Absolute comparison with public Default and pinned libwebp m6 streams

These are separate baselines and are not described as same-profile gains.
The decode table uses one comparison binary and the **project Rust decoder**
for all five stream layouts. In particular, the `libwebp m6` row means that
the Rust decoder is decoding streams generated by pinned libwebp method 6; it
is not a pinned C decoder timing.

| stream layout decoded by project Rust | total bytes | vs public Default bytes | vs pinned m6 bytes | Rust decode median | vs Default decode | vs m6-stream decode |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| public Default | 661,692,326 | — | +149.674% | 5.002243 s | — | -15.763% |
| Compact | 617,958,802 | -6.609% | +133.174% | 4.034269 s | -19.351% | -32.064% |
| LowLatency | 625,321,072 | -5.497% | +135.952% | 4.009531 s | -19.845% | -32.481% |
| pinned libwebp m6-generated streams | 265,020,980 | -59.947% | — | 5.938344 s | +18.713% | — |

Thus the product profiles are substantially larger than libwebp method 6,
while the project Rust decoder processes these coarse fast-profile streams
faster than either public Default or the pinned m6-generated streams.

## Pinned C WebPDecodeRGBA cross-stream timing

The actual pinned C decoder was measured separately; this is a product
cross-stream comparison, not a rerun or redefinition of the Rust decoder gate.
One C binary linked against pinned libwebp
`733c91e461c18cf1127c9ed0a80dccbcfed599d3` decoded all four layouts. The
runner held its own atomic lock, preloaded encoded and expected files, alternated
forward/reverse order, and retained five rounds. Timed work includes
`WebPDecodeRGBA` allocation/materialization, full RGBA `memcmp`, and full RGBA
FNV-1a.

| stream layout decoded by pinned C | median wall | MAD | vs m6 streams | paired median | median CPU | median max RSS | wall outliers |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| pinned libwebp m6-generated streams | 5.965627 s | 0.004489 s | — | — | 6.126984 s | 1,301.30 MiB | r4 |
| public Default | 5.432180 s | 0.007423 s | -8.942% | -8.942% | 5.685760 s | 1,678.09 MiB | r2 |
| Compact | 5.335206 s | 0.001505 s | -10.568% | -10.576% | 5.570309 s | 1,636.69 MiB | r1/r2 |
| LowLatency | 5.279929 s | 0.013647 s | -11.494% | -11.308% | 5.531357 s | 1,643.20 MiB | none |

RSS includes all preloaded encoded and expected RGBA buffers and therefore is
useful only as a like-for-like cross-layout process measurement. All flagged
rounds remain in the headline medians; detailed CPU/RSS/outlier records are in
`libwebp-decode-102/summary.json` and `processes.jsonl`.

## Validation

Stable Rust 1.97.1 passed:

- workspace `cargo test --workspace --all-targets` in debug and release;
- workspace `cargo clippy --workspace --all-targets -- -D warnings`;
- workspace formatting check and `git diff --check`;
- `webp` rustdoc with `-D warnings` and the public API doctest;
- `webp` compile checks for `wasm32-unknown-unknown`,
  `x86_64-pc-windows-gnu`, and `x86_64-pc-windows-msvc`;
- C helpers with `-Wall -Wextra -Werror` syntax checks;
- Python helper bytecode compilation and reproducer `bash -n`.

No nightly toolchain, unsafe Rust, new dependency, or production concurrency
was introduced. The committed raw evidence includes all measurement files,
process records, robust summaries, exact stream census, decoder oracle output,
default before/after hashes, and stream-identity audit.

Primary evidence SHA-256:

- `streams-102.tsv`:
  `57961c33496d674b614b88c1c5ec33eee79f63baf20cf9780da0fc6a1ba29ec1`
- `oracle-408.tsv`:
  `c6c113966bdb81ded723112da67575439e46b0c1a9cd6245baa4579abfb0197c`
- `stream-identity-306.tsv`:
  `55c00684a42083d39af4ad25b10def2d0eb54583930ca1d08adeb45a3c2ab774`
- formal `processes.jsonl`:
  `bc5d4c698ee4bfe518752af9cb8b604d82758b147647cd0879ab772c57664a8b`
- formal `summary.json`:
  `fe2b1a43670694f9ad3cfe639020ae457d04b7ed8be2252502ded8d26649f53c`
- comparative `summary.json`:
  `68bf62d79c2bd83cef0e4a6b323077d2343948f2311a742149ddbca6f8878e78`
- pinned C decoder `processes.jsonl`:
  `158e9ce2af1fb1099bb31812dd81758ea8274eed99c14bd29398fd39a1633432`
- pinned C decoder `summary.json`:
  `03f331042ddbb2da95afe5030882b4271b118f41857f6b4542b23251ef8aa9e5`

See `reproduce.sh` for the current-tree generation, pinned oracle, screen,
formal gate, comparative decode, and summarization commands.
