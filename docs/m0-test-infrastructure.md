# M0: deterministic test and corpus infrastructure

M0 establishes the contract that later codec work is measured against.  It
does **not** claim that any WebP bitstream is decoded yet.

## What exists

- `crates/webp-testkit`: safe-Rust manifest types, TOML parser, SHA-256
  integrity verification, and deterministic sidecar discovery.
- `tests/fixtures/smoke`: a committed minimal malformed-input seed.
- `tests/manifests`: data-driven expected classifications and resource budgets.
- `tools/corpus-lock.toml`: reviewed, immutable pins for the libwebp oracle and
  upstream conformance corpus. It records both Git revisions and archive
  checksums; it must never name a moving branch.
- `tools/faults`: the record format for deliberate fault-injection checks.

## Adding a fixture

1. Put fixture bytes in the appropriate corpus directory.
2. Create one TOML manifest in `tests/manifests/` and calculate its SHA-256.
3. State whether it must be accepted, rejected, accepted only by the
   compatibility profile, or is implementation-defined.
4. For accepted inputs, record dimensions, canonical RGBA hash when available,
   resource budgets, source, and license.
5. Wire the manifest into a public-path test using `FixtureRunner`; the test
   callback selects the API and asserts the expected classification.

The runner is rooted at `tests/`, recursively discovers manifests, validates
their resolved paths remain under that root, and verifies bytes before invoking
the callback.  This prevents an accidentally edited fixture from changing the
meaning of a golden test without a manifest update.

## Local verification

From the repository root, run:

```text
cargo test -p webp-testkit
```

Until the root workspace includes the testkit crate, it can be tested directly:

```text
cargo test --manifest-path crates/webp-testkit/Cargo.toml
```

The upstream test data and the oracle are deliberately ignored by Git. Fetch
them only at their locked revisions under `third_party/`; committed PR smoke
fixtures must stay small and independently licensed.

The first decoder integration test should run the smoke manifest and call each
selected public API (`read_info`, one-shot decode, and incremental finish).
