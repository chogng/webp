# M1 validation record

This record summarizes the local M1 VP8L decoder gate run on 2026-07-20.

## Locked references

- libwebp oracle: `733c91e461c18cf1127c9ed0a80dccbcfed599d3`
- libwebp-test-data: `06ddd96e276c2c638a72d39d3c0f340afd61978c`
- selected upstream files: 68, each verified by SHA-256
- M1 `MustAccept` VP8L files: 41, each checked against canonical RGBA8

## Decoder gates

- All 41 selected VP8L files decode with the public Rust decoder and match the
  pinned libwebp dimensions and RGBA SHA-256.
- All 16 `lossless_vec_1_*` files complete whole-file and rewrapped VP8L-payload
  truncation at every byte boundary without a panic.
- Thirty-two deterministic small RGBA images encoded by pinned `cwebp` are
  recovered byte-for-byte by both the Rust decoder and pinned `dwebp`.
- The 16 official transform combinations are present in the accepted corpus.
- Raw VP8L, structured Huffman, and structured inverse-transform fuzz targets
  each completed a 10,000-run sanitizer smoke test without a finding.

The encoder-produced half of the bidirectional round trip remains an M4 gate:
M1 has no encoder API or implementation.

## Performance status: pending

M1 met its conformance and robustness gates, but it did not originally have a
performance exit gate. The repeatable baseline is now
`bash tools/benchmark-vp8l.sh 5`; see [`quality-gates.md`](quality-gates.md).
On the 41 `MustAccept` VP8L inputs, the local median was 1.894 s for the Rust
public decoder and 0.518 s for the same libwebp C-API work, a 3.65x gap.

M1 must not be described as fully complete until a profile identifies the
dominant remaining hot paths and the remediation work has an accepted
performance target.

## Critical mutation results

| Package | Generated | Caught | Timeout | Equivalent/unreachable | Unviable |
| --- | ---: | ---: | ---: | ---: | ---: |
| `webp-core` | 108 | 100 | 0 | 4 | 4 |
| `webp-vp8l-huffman` | 85 | 80 | 0 | 0 | 5 |
| `webp-vp8l-entropy` | 105 | 103 | 2 | 0 | 0 |
| `webp-vp8l-transform` | 149 | 148 | 0 | 0 | 1 |
| `webp-vp8l-color-transform` | 44 | 41 | 0 | 0 | 3 |
| `webp-vp8l-indexing` | 51 | 46 | 0 | 1 | 4 |

The conservative score is 518 caught out of 525 viable mutants (98.7%). The
two entropy timeouts replace a decreasing loop counter with a non-converging
operation and are classified as detected liveness failures. The five surviving
mutants are explained invariants, unreachable diagnostics, or reserve-capacity
changes with no public result change; there are no unexplained survivors.

## Final commands

```sh
tools/verify-upstream-smoke.sh
cargo test --release --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
cargo +nightly-2026-07-15 fuzz run vp8l_raw -- -runs=10000
cargo +nightly-2026-07-15 fuzz run vp8l_huffman -- -runs=10000
cargo +nightly-2026-07-15 fuzz run vp8l_transforms -- -runs=10000
bazel test --test_output=errors --test_tag_filters=external-corpus \
  //webp-rs/webp:external_upstream_corpus_test
```

The Bazel external-corpus target passed locally with Bazel 9.0.0. Scheduled CI
builds pinned `cwebp`/`dwebp`, reruns the release external-corpus test, and
runs the same Bazel target.
