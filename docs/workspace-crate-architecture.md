# WebP sibling-crates workspace architecture

The repository root `webp-rs/` is a Cargo workspace. Production packages are
direct children of that root; there is deliberately no intermediate `crates/`
directory.

```text
webp-rs/
├── webp/       unified compatibility API and current codec orchestration
├── decode/     package webp-decode: decode-only public surface
├── encode/     package webp-encode: encode-only public surface
├── container/  package webp-container: shared WebP RIFF vocabulary
├── demux/      package webp-demux: zero-copy container parsing
├── mux/        package webp-mux: container construction and editing
├── dsp/        package webp-dsp: pure pixel-domain kernels
├── sharpyuv/   package webp-sharpyuv: reconstruction-aware color conversion
├── utils/      package webp-utils: small format-neutral infrastructure
└── xtask/      workspace maintenance commands
```

## Ownership rules

- `container` defines what shared WebP container fields mean. It does not read
  or write complete containers.
- `demux` owns byte parsing, compatibility policy, resource limits, borrowed
  chunks, and parsed animation models.
- `mux` owns output allocation, owned chunks, serialization, and lossless
  editing. Its editor composes the public demux and mux capabilities.
- `dsp` owns stateless pixel kernels used in both directions. It cannot depend
  on a codec orchestrator.
- `sharpyuv` owns RGB-to-YUV reconstruction-aware sampling and cannot depend on
  the VP8 encoder.
- `utils` contains only format-neutral primitives with multiple consumers. A
  helper that mentions VP8X, ALPH, ANMF, or a codec error remains with its
  domain owner.
- `decode` and `encode` provide direction-specific dependency surfaces. The
  existing `webp` package remains source-compatible while codec bodies are
  incrementally transferred behind those surfaces.

## Current codec ownership

The public direction boundary is now implemented, rather than being only a
facade split:

- `decode` owns static and animated decoding, incremental decoding, inspection,
  decode limits, and the read side of `ALPH` payload handling.
- `encode` owns the public static-image and animation encode orchestration,
  lossless/lossy options, VP8 source conversion and frame writing, VP8L entropy
  and spatial planning, and the complete `ALPH` encoder (filters, level
  reduction, palette planning, LZ77 planning, entropy planning, and packed
  token output). Its codec-local tests live alongside those owners.
- `container` also owns the shared `ALPH` header vocabulary used by both
  direction-specific payload implementations.
- `utils` owns the format-neutral least-significant-bit-first `BitWriter`.
  It returns its own small error type and does not depend on WebP codec errors.
- `decode` retains only the VP8 reader and codec primitives. Its narrowly
  scoped `#[doc(hidden)] webp_decode::vp8_codec` surface is an unstable
  temporary home for algorithms shared by that reader and the `encode`-owned
  writer; it contains no writer state or source conversion.

The compatibility crate routes `decode` and `encode` features independently.
In particular, `webp`'s `decode` feature forwards `webp-decode/decode`, so a
decode-only dependency receives the same public models and limits as the
default build.

## Dependency direction

```text
webp-container <- webp-demux <- decode consumers
webp-container <- webp-mux   <- encode consumers
webp-utils     <- demux / mux / codec orchestration
webp-dsp       <- codec orchestration
webp-sharpyuv  <- VP8 encoding
webp-decode    <- webp-encode (temporary private VP8 codec-primitives bridge)
```

Cycles between production crates are forbidden. Source relocation is accepted
only when the destination owns the state, algorithms, tests, and invariants;
moving a file without changing its dependency direction is not considered an
architectural extraction.

## Validation for the direction split

Run the ordinary workspace matrix and the two public feature boundaries:

```sh
cargo +stable test --workspace --all-features
cargo +stable check -p webp --no-default-features --features decode
cargo +stable check -p webp --no-default-features --features encode
```

The private `alpha_writer_identity` integration test additionally requires the
external 41-file/15-file corpora and a pinned `dwebp` binary through its
documented environment variables. It prints an explicit skip and succeeds when
those fixtures are absent, while a provisioned host runs the full identity
matrix.
