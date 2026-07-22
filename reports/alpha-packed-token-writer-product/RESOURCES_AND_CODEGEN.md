# Resources, artifacts, and code generation

## Formal process resources

| Variant | CPU rounds (s) | Median | Peak RSS rounds (bytes) | Median |
| --- | --- | ---: | --- | ---: |
| Reference | 8.515559, 8.408838, 8.402176, 8.396349, 8.410831 | 8.408838 | 111083520, 110854144, 110919680, 110854144, 110870528 | 110870528 |
| Packed | 8.077830, 8.072113, 8.073714, 8.069629, 8.077725 | 8.073714 | 106774528, 106823680, 101613568, 106840064, 106807296 | 106807296 |

Packed changes median CPU by `-3.985%` and median peak RSS by `-3.665%`.
Raw `/usr/bin/time -l` observations and parsed values are in
`raw/formal-41-final/measurements/*.stderr` and `processes.json`.

## Artifact sizes

Isolated default release builds used the registered base worktree and product
worktree with separate target directories.

| Artifact | Base | Product | Delta |
| --- | ---: | ---: | ---: |
| `alpha_encode_bench` | 774,992 | 775,152 | +160 (+0.021%) |
| `libwebp_alpha.rlib` | 244,328 | 281,272 | +36,944 (+15.121%) |

The benchmark-only feature is not part of either default artifact. Exact paths
and SHA-256 values are in `raw/binary-artifacts.tsv`.

## Optimized ownership

The product alpha crate was emitted as optimized LLVM IR with stable Rust. The
IR names the hot ownership directly:

- `webp_alpha::encode_token_output::write_tokens`
- `webp_alpha::encode_token_output::write_tokens_with_variant::{closure#0}`
- `webp_alpha::encode_token_output::TokenPacket::push`
- `webp_alpha::encode_token_output::PackedTokenSink::{from_prefix,append,finish}`

In the textual span for optimized `write_tokens`, the only grow operation is
the inlined up-front capacity reservation in `PackedTokenSink::from_prefix`
(one comment and its one call). No grow/reserve appears on the token-loop back
edges and no `BitWriter::write_bits` call appears in the packed traversal.
The loop calls the packet-composition closure and appends the resulting at-most
58-bit packet to the persistent sink.

This is codegen evidence, not a replacement for behavior tests: width,
crossing, tail, padding, overflow, capacity, cached/replayed order, and byte
identity are independently tested.
