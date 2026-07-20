# webp-rs

A safe-Rust WebP implementation, built from a test-first plan. The current
milestone is M0: hardened RIFF/WebP container parsing, resource limits, and
reproducible fixture infrastructure. Codec decoding is intentionally not yet
implemented.

## Current guarantees

- Core crates forbid `unsafe` code.
- Container parsing uses checked size and padding arithmetic.
- Strict and libwebp-compatible parsing policies are explicit.
- Metadata can be inspected without allocating pixel buffers.

Run the M0 test suite with:

```sh
cargo test --workspace
```

