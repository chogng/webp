# webp-rs

A safe-Rust WebP implementation, built from a test-first plan. The current
milestone is M2 foundation work: the M1 VP8L decoder is functionally complete
but performance pending, while the
lossy VP8 path now has hardened key-frame/header validation and public image
information. VP8 entropy, reconstruction, filtering, and pixel output are not
yet implemented.

## Current guarantees

- Core crates forbid `unsafe` code.
- Container parsing uses checked size and padding arithmetic.
- Strict and libwebp-compatible parsing policies are explicit.
- Metadata can be inspected without allocating pixel buffers.
- VP8L headers, canonical Huffman tables, LZ77 copy, and predictor primitives
  are connected to the static VP8L public pixel decoder.
- VP8 frame tags, key-frame start codes, dimensions, VP8X canvas agreement,
  and first-partition boundaries are checked before entropy-state allocation.
- VP8 boolean entropy values and fixed-width literals have bounded,
  deterministic decoding primitives plus a dedicated fuzz target.
- VP8 `read_info` works for unextended still-image containers; VP8 `decode`
  explicitly reports that pixel decoding is pending.

Run the workspace test suite with:

```sh
cargo test --workspace
```

See [the test-organization guide](docs/test-organization.md) for the required
split between module-private `*_tests.rs` files and public-API integration
tests in each crate's `tests/` directory.

Codec milestones also require the conformance, robustness, performance, and
resource gates in [`docs/quality-gates.md`](docs/quality-gates.md); passing
the test suite alone does not mark a decoder milestone complete.

## Bazel

The workspace also supports Bazel through Bzlmod and `rules_rust`. Bazel uses
the checked-in Cargo manifests and lockfiles to resolve third-party crates, so
Cargo remains the dependency source of truth.

```sh
bazel test --test_output=errors --test_tag_filters=-external-corpus //...
```

Use Bazelisk to select the pinned version in `.bazelversion`. Bazel 9 maintains
the Bzlmod graph lock and the Rust crate-universe lock separately. Update the
Bzlmod lock after changing `MODULE.bazel`:

```sh
bazel mod deps --lockfile_mode=update
```

When Cargo dependencies change, regenerate the crate-universe lock during a
normal Bazel analysis before committing:

```sh
CARGO_BAZEL_REPIN=1 bazel build //...
```

Verify both dependency graphs without update mode:

```sh
bazel mod deps --lockfile_mode=error
bazel build //...
```

External-corpus tests are marked `manual` and `external-corpus`: ordinary
Bazel test runs explicitly exclude them, remain offline, and run only source
tests plus committed fixtures. The whole `third_party/` directory is ignored:
it holds only downloaded validation material, including the pinned
`libwebp-test-data` corpus. CI jobs that fetch that corpus opt into the
`external-corpus` tag and run the relevant explicit target with it declared as
a Bazel runfile. See `docs/test-corpus.md` for the locked download and
validation workflow.

## License

Licensed under either of:

- [Apache License, Version 2.0](LICENSE-APACHE)
- [MIT License](LICENSE-MIT)
