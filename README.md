# webp-rs

A safe-Rust WebP implementation, built from a test-first plan. The current
milestone is M5+ encoding: static VP8L and VP8 key frames decode to straight
RGBA8, including `ALPH` transparency, and animated containers decode to
display-ready canvas frames. M4 begins with safe, static lossless VP8L literal
encoding; performance work remains deliberately deferred.

## Current guarantees

- Core crates forbid `unsafe` code.
- Container parsing uses checked size and padding arithmetic.
- Strict and libwebp-compatible parsing policies are explicit.
- Metadata can be inspected without allocating pixel buffers.
- VP8L headers, canonical Huffman tables, LZ77 copy, and predictor primitives
  are connected to the static VP8L public pixel decoder.
- VP8 key frames decode through bounded entropy, reconstruction, loop
  filtering, and YUV-to-RGBA conversion.
- `webp-alpha` owns complete `ALPH` payload encoding and decoding: raw and
  headerless-VP8L compression, fixed/fast/best spatial-filter selection,
  quality-driven level reduction, compressed-size comparison with raw
  fallback, LZ77 and frequency-derived Huffman coding, preprocessing/header
  fields, and bounded decode resources. Strict container parsing checks alpha
  feature flags in static and animated containers.
- Animated `ANIM`/`ANMF` containers validate frame geometry and resources;
  `decode_animation` returns full display-order RGBA canvas snapshots after
  blend and disposal.
- External libwebp vectors cover ALPH filters and animated blend/dispose
  composition, including pixel-level oracle checks where representations match.
- `encode_lossless_rgba` writes a complete static VP8L WebP from straight
  RGBA8 input using reversible transforms, bounded adaptive color-cache
  selection, small-palette color indexing, and deterministic frequency-ranked
  Huffman coding. Metadata can be muxed through
  `encode_lossless_rgba_with_metadata`.
- `encode_lossless_animation` writes strict `VP8X`/`ANIM`/`ANMF` containers
  with independently encoded VP8L frame rectangles, including timing, even
  offsets, blend, dispose-to-background, alpha, canvas background, and loops.
  `encode_lossless_animation_with_metadata` additionally preserves raw ICCP,
  EXIF, and XMP payloads.

Run the workspace test suite with:

```sh
cd webp-rs && cargo test --workspace
```

Compare the lossy-RGB/lossless-ALPH encoder against the pinned libwebp
public API over the upstream transparent corpus with:

```sh
tools/benchmark-alpha-encode.sh 50
```

Codec milestones also require the conformance, robustness, performance, and
resource gates in [`docs/quality-gates.md`](docs/quality-gates.md); passing
the test suite alone does not mark a decoder milestone complete.
M3's functional exit record is in
[`docs/m3-alpha-animation.md`](docs/m3-alpha-animation.md); M4's completed
static VP8L encoding scope and exit criteria are in
[`docs/m4-vp8l-encoding.md`](docs/m4-vp8l-encoding.md). The follow-on encoder
roadmap and completed M5/M8 profiles are in
[`docs/m5-plus-roadmap.md`](docs/m5-plus-roadmap.md).

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

When Cargo dependencies change, update the committed workspace lock and verify
that Bazel resolves it:

```sh
cd webp-rs && cargo update
cd .. && bazel build //...
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
