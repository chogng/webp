# Third-party benchmark references

This directory owns durable results for pinned external implementations. It
does not contain upstream source checkouts, builds, corpora, generated streams,
raw timing rows, or temporary binaries.

Each reference is recorded once for a fixed corpus, host, and measurement
contract. Project candidates read that result for horizontal comparison.
Replacing an external reference is allowed only after its pinned revision or
measurement contract changes.

The VP8L encoder workflow uses:

- `libwebp/vp8l-encode.md` for the fixed pinned-libwebp reference, current
  accepted Rust result, and derived horizontal comparison;
- `tools/benchmark-vp8l-reference.sh` to establish or explicitly replace the
  upstream reference;
- `tools/benchmark-vp8l-encode.sh` to measure each Rust candidate once.
