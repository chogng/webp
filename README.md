# webp-rs

A safe-Rust WebP implementation, built from a test-first plan. The current
milestone is M3 functional integration: static VP8L and VP8 key frames decode
to straight RGBA8, including `ALPH` transparency, and animated containers
decode to display-ready canvas frames. Performance work remains deliberately
deferred until the remaining functional scope is complete.

## Current guarantees

- Core crates forbid `unsafe` code.
- Container parsing uses checked size and padding arithmetic.
- Strict and libwebp-compatible parsing policies are explicit.
- Metadata can be inspected without allocating pixel buffers.
- VP8L headers, canonical Huffman tables, LZ77 copy, and predictor primitives
  are connected to the static VP8L public pixel decoder.
- VP8 key frames decode through bounded entropy, reconstruction, loop
  filtering, and YUV-to-RGBA conversion.
- `ALPH` supports raw and headerless-VP8L compression with all four spatial
  filters; strict parsing checks alpha feature flags in static and animated
  containers.
- Animated `ANIM`/`ANMF` containers validate frame geometry and resources;
  `decode_animation` returns full display-order RGBA canvas snapshots after
  blend and disposal.
- External libwebp vectors cover ALPH filters and animated blend/dispose
  composition, including pixel-level oracle checks where representations match.

Run the workspace test suite with:

```sh
cargo test --workspace
```

Codec milestones also require the conformance, robustness, performance, and
resource gates in [`docs/quality-gates.md`](docs/quality-gates.md); passing
the test suite alone does not mark a decoder milestone complete.
M3's functional exit record is in
[`docs/m3-alpha-animation.md`](docs/m3-alpha-animation.md).

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
