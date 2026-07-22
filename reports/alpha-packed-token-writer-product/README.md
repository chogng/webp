# A14: product packed-token writer

## Decision

**Recommend integration into `main`.** On the registered latest-main base, the
productionized writer preserves every tested byte and decoder result while the
formal 41-file ALPH-only aggregate improves from 816.001 ms to 659.907 ms
(`-19.129%`). This clears the `>=10%` promotion threshold with no material
whole-encoder or peak-RSS regression.

The recommendation is deliberately limited to the 41-file headline corpus.
The separate 15-file generalization result is `-8.063%` overall and only
`-3.825%` on its four real files. A13 had the same limitation (`-7.078%`
overall, `-2.955%` real, `-9.307%` synthetic), plus retained small-file
regressions. Those results are not blended into the headline.

## Provenance

| Role | Commit |
| --- | --- |
| Registered measurement base | `1c16ebe826ea57adaf2293bf44bdc36175401a8b` |
| Product code | `77842c1c2021cc76a58ce283c4fa42604cab4048` |
| Evidence/report | recorded by the following evidence commit |
| Final | branch tip after the final audit commit |

The product code was migrated manually from the read-only A13 result. No A13
commit was cherry-picked and no A13 ancestry is present. Before the screen,
formal run, and final recommendation, local `main` was re-read as
`1c16ebe826ea57adaf2293bf44bdc36175401a8b`; it is the merge-base and an
ancestor of the product branch.

The final same-binary measurements used release example SHA-256
`6cddb10469cafe6f3e0fec7eb9a6cf8ebd9a6a509108373ff6d2b02b55224c03`.
Each result directory records that hash, the binary path, iterations, rotation
order, lock path, process resources, all per-file observations, and identity
checks in `run.json`, `manifest.tsv`, `processes.json`, and `summary.json`.

## Product architecture

The dependency direction is:

```text
encode (validation/filter/palette, frequencies, tables)
  -> encode_token_output (variant, traversal, packet composition, sink)
       -> encode_lz77 (Token, walk, VP8L prefix syntax and alphabets)
       -> encode_huffman (encoding tables and wire symbols)
```

Imports demonstrate the graph: `encode.rs` imports and delegates to
`encode_token_output`; that owner imports `encode_lz77` and `encode_huffman`;
neither lower module imports private encode orchestration details. `lib.rs`
only declares private modules and explicitly reexports the existing public API
plus doc-hidden, feature-gated benchmark controls.

Production module sizes are 294 lines for `encode.rs`, 368 for
`encode_token_output.rs`, and 206 for `encode_lz77.rs`, all below the practical
500-line target. New invariant tests live in the descriptive sibling
`encode_token_output_tests.rs` via `#[path]`.

The concrete token-output owner contains:

- reference, packet-reference, and packed writer selection;
- cached-token and replayed greedy-token traversal in the same order;
- complete literal/copy packet composition;
- the persistent packed sink, prefix-tail reconstruction, low-word flush, and
  final zero padding.

Only `Packed` exists in ordinary builds. The two controls exist solely behind
the nondefault `benchmark-internals` / `alpha-benchmark-internals` feature,
are doc-hidden, thread-local, and do not expand the default public API or
runtime. A13 census hooks and experiment scaffolding were not migrated.

## Bit invariant and safety boundary

A literal packet is at most 15 bits, the maximum green Huffman code width. A
copy packet is at most 58 bits:

```text
length symbol 15 + length extra 10 + distance symbol 15 + distance extra 18
```

The packet is composed and appended least-significant bit first in exactly
that order: length symbol, length extra, distance symbol, distance extra. The
focused tests cover widths 0 through 58, offsets 0 through 7, word crossings,
prefix tails, zero-width codes, maximum extras, the 15-bit literal and 58-bit
copy maxima, low-word flushes, zero padding, checked capacity, allocation
failure mapping, and arithmetic overflow.

The sink reconstructs the partially occupied prefix byte, reserves the
checked worst-case token capacity once, writes complete low `u32` words in
little-endian order, and emits only the occupied final bytes. Checked
arithmetic maps overflow to `SizeOverflow`; failed reservation maps to
`AllocationFailed`. Filter/preprocess/palette selection, greedy tokenization,
hash overwrites, cached-token order, frequencies, Huffman tables/codes,
fallback/selector behavior, public API, and public error behavior are
unchanged.

Release LLVM IR places `write_tokens`, `TokenPacket`, and `PackedTokenSink`
under `encode_token_output`. Within the optimized `write_tokens` body there is
one inlined grow call for the up-front reserve (one comment plus one call in
the textual IR), no per-token reserve/grow call, and no `BitWriter::write_bits`
call in the packed traversal. See `RESOURCES_AND_CODEGEN.md`.

## Valid performance evidence

All timings are locked, preloaded, single-process, same-binary medians. A
positive percentage is a regression. The packet-reference (`P`) control uses
the old persistent bit writer with the new packet composition, isolating the
packed sink benefit.

### Screen: 41 files, 10 encodes, 3 rotations

| Profile | Reference | P | Packed | Packed change |
| --- | ---: | ---: | ---: | ---: |
| ALPH only | 832.087 ms | 837.499 ms | 670.273 ms | **-19.447%** |
| Whole Rust encoder | 6,916.822 ms | 6,886.425 ms | 6,764.310 ms | **-2.205%** |

The screen had zero byte/hash mismatches. ALPH per-file tails were p5
`-15.946%`, p50 `-6.542%`, p95 `+1.010%`, best `-24.506%`, worst `+5.051%`,
with 3/41 regressions. Whole tails were p5 `-5.785%`, p50 `-1.317%`, p95
`+3.352%`, best `-8.719%`, worst `+9.967%`, with 14/41 regressions. The
aggregate passed, so the formal and generalization gates ran.

### Formal headline: 41 files, 10 encodes, 5 rotations

| Profile | Reference median (MAD) | P median | Packed median (MAD) | Packed change |
| --- | ---: | ---: | ---: | ---: |
| ALPH only | 816.001 (1.794) ms | 825.730 ms | 659.907 (0.799) ms | **-19.129%** |
| Whole Rust encoder | 6,798.402 ms | 6,793.995 ms | 6,636.077 (3.211) ms | **-2.388%** |

ALPH reference rounds were 827.016, 817.707, 816.001, 814.000, and
814.207 ms; packed rounds were 659.907, 662.455, 658.929, 659.330, and
660.706 ms. The P control was `+1.192%`, confirming that packet composition
alone is not the optimization.

Formal ALPH tails were p5 `-15.563%`, p50 `-5.000%`, p95 `-3.061%`, best
`-24.037%`, worst `-2.948%`, with 0/41 regressions. Whole tails were p5
`-3.532%`, p50 `-1.119%`, p95 `-0.337%`, best `-3.613%`, worst `+1.047%`,
with 1/41 regression. There were no size or hash mismatches.

Median process CPU fell from 8.408838 s to 8.073714 s (`-3.985%`). Median
peak RSS fell from 110,870,528 to 106,807,296 bytes (`-3.665%`).

### Separate generalization: 4 real + 11 synthetic, 5 encodes, 5 rotations

| Group | ALPH reference | ALPH packed | Change | Whole change |
| --- | ---: | ---: | ---: | ---: |
| All 15 | 396.341 ms | 364.385 ms | **-8.063%** | -0.685% |
| Real 4 | 153.401 ms | 147.534 ms | **-3.825%** | -0.161% |
| Synthetic 11 | 242.921 ms | 216.870 ms | **-10.724%** | -0.921% |

All ALPH files improved: p5 `-19.231%`, p50 `-4.607%`, p95 `-1.377%`, best
`-19.730%`, worst `-0.902%`. Two synthetic whole-file cases regressed by at
most `+0.028%`. This corpus does not meet the headline threshold and remains a
material limitation.

## Identity and compatibility

The final 56-file matrix at q0/q70/q99/q100 produced **224/224** exact pairs:

- reference and packed ALPH bytes matched;
- reference and packed complete WebP bytes matched;
- the project decoder reconstructed the input alpha plane;
- pinned `dwebp` reconstructed the input alpha plane.

The pinned libwebp comparison is used only at the whole-encoder boundary. Over
three 41x10 runs, Rust whole-encoder aggregates were 6,899.961, 6,893.383, and
6,894.267 ms; libwebp aggregates were 9,765.507, 9,752.862, and 9,752.954 ms.
The median boundary is 6,894.267 versus 9,752.954 ms (`-29.311%`). These are
not byte-comparability claims against libwebp.

## Build, lint, test, and artifact gates

The final code passes:

- focused alpha tests in debug/release, default and benchmark feature builds;
- `cargo test --workspace` in debug and release;
- workspace, alpha-feature, and webp-feature Clippy with `-D warnings`;
- `cargo fmt --all -- --check`;
- affected Bazel alpha/webp unit tests and `alpha_encoder_oracle_test`;
- all existing nightly fuzz-target builds;
- the 224-case identity matrix with pinned `dwebp`.

No unsafe code, dependency, or ordinary nightly requirement was added.

The isolated default release example changed from 774,992 to 775,152 bytes
(`+160`, `+0.021%`). The alpha rlib changed from 244,328 to 281,272 bytes
(`+36,944`, `+15.121%`), primarily because the private owner and both error
paths are retained at the crate boundary. Exact hashes are in
`raw/binary-artifacts.tsv`.

## Evidence map

- `raw/screen-41-final/`: valid 41x10x3 screen and all per-file tails.
- `raw/formal-41-final/`: valid 41x10x5 headline, CPU, RSS, and tails.
- `raw/generalization-15-final/`: valid 15x5 generalization and group summary.
- `raw/identity-56-q-matrix-final.log`: valid 224-case byte/decoder matrix.
- `raw/gates/`: final build, lint, Bazel, fuzz, codegen, artifact, and libwebp logs.
- `raw/invalidated/pre-lint-amend/`: every pre-amend run, retained but excluded.
- `FAILURES.md`: invalid runs and corrected command failures.
- `SHA256SUMS`: checksum manifest for the report tree.
