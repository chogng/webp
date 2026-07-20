# External corpus adapters

Files beneath `third_party/` are never called directly by codec tests. Each
external corpus has a manifest directory (committed under `tests/corpora/`, or
generated beside ignored data when every WebP is regenerated) and is executed
with `FixtureRunner::with_fixture_root(manifest_root, fixture_root)`.

That gives the Rust test path the same guarantees as committed fixtures:

- SHA-256 verification before an API receives bytes;
- explicit classification, selected API, expected hashes, and limits;
- containment under the fixed external corpus root;
- no shell tool or network request at test execution time.

`crates/webp-testkit/tests/external_reference_corpus.rs` proves that generated
reference sidecars are consumable by Rust: it discovers manifests, enforces
path containment, and verifies every input SHA-256 before a decoder test can
consume bytes. A WebP file without its sidecar is not eligible for a Rust
conformance or regression test.
