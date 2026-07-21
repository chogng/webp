# M0: deterministic test and corpus infrastructure

M0 establishes the contract that later codec work is measured against.  It
does **not** claim that any WebP bitstream is decoded yet.

## What exists

- `tests/fixtures/smoke`: a committed minimal malformed-input seed.
- `webp-rs/webp` integration tests: direct fixture and corpus consumption through
  the public Rust API.
- `tools/corpus-lock.toml`: reviewed, immutable pins for the libwebp oracle and
  upstream conformance corpus. It records both Git revisions and archive
  checksums; it must never name a moving branch.
- `tools/faults`: the record format for deliberate fault-injection checks.

## Adding a fixture

1. Put fixture bytes in the appropriate corpus directory.
2. Add a direct public-API test in `webp-rs/webp` that reads the fixture and
   asserts the expected result.
3. Keep any required dimensions, pixels, or error expectation alongside that
   test so its contract is visible at the call site.

## Local verification

From the repository root, run:

```text
cd webp-rs && cargo test -p webp
```

The complete `third_party/` directory is deliberately ignored by Git. It holds
only downloaded test data, the optional libwebp oracle, and benchmarks. Fetch
the libwebp test data only at its locked revision; committed PR smoke fixtures
must stay small and independently licensed.

To fetch the upstream conformance vectors, run:

```text
tools/fetch-libwebp-test-data.sh
```

The script rejects an existing checkout with a different `origin` and confirms
that its detached `HEAD` is the `libwebp_test_data.commit` in
`tools/corpus-lock.toml`.

Fetch the pinned, test-only libwebp oracle with:

```text
tools/fetch-libwebp-oracle.sh
```

The overall five-source corpus policy and profile gates are in
`docs/test-corpus.md`.

The 64-file feature smoke selection is versioned separately from the downloaded
binary corpus. Validate its completeness after fetching with:

```text
tools/verify-upstream-smoke.sh
```

It is the candidate PR vector list once the public decoder can accept valid
VP8L and VP8 inputs; until then it remains a verified external corpus rather
than a falsely passing decode test.

Generate the committed structural-malformation corpus with:

```text
cd webp-rs && cargo run -p xtask -- fixtures generate-malformed
```

The generator is idempotent; direct API tests classify each generated input.

`[clic]` pins the benchmark data identity and its validation split. The actual
images belong in the ignored `third_party/benchdata/clic/` directory and are
used only after encoding/decoding benchmarks exist; they are not conformance
fixtures or decoder golden outputs.

Decoder integration tests read the smoke fixtures directly and call each
selected public API (`read_info`, one-shot decode, and incremental finish).
