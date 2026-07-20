# webp-rs

A safe-Rust WebP implementation, built from a test-first plan. The current
milestone is M1 foundation work: hardened RIFF/WebP container parsing,
resource limits, reproducible fixture infrastructure, and independently tested
VP8L header/entropy/transform primitives. Full codec decoding is intentionally
not yet implemented.

## Current guarantees

- Core crates forbid `unsafe` code.
- Container parsing uses checked size and padding arithmetic.
- Strict and libwebp-compatible parsing policies are explicit.
- Metadata can be inspected without allocating pixel buffers.
- VP8L headers, canonical Huffman tables, LZ77 copy, and predictor primitives
  are individually checked but are not yet connected to a public pixel decoder.

Run the M0 test suite with:

```sh
cargo test --workspace
```
