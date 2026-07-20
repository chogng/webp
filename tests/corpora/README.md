# External corpus adapters

Files beneath `third_party/` are never called directly by Rust tests. Each
external corpus gets a committed manifest directory under `tests/corpora/` and
is executed with `FixtureRunner::with_fixture_root(manifest_root, fixture_root)`.

That gives the Rust test path the same guarantees as committed fixtures:

- SHA-256 verification before an API receives bytes;
- explicit classification, selected API, expected hashes, and limits;
- containment under the fixed external corpus root;
- no shell tool or floating upstream branch at test execution time.

Reference-encoder outputs additionally record the libwebp revision, exact
encoder arguments, source-image hash, and oracle-decoded RGBA hash. A WebP file
without this sidecar is not eligible for a Rust conformance or regression test.
