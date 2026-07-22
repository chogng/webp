# Decoder-only product profile

`webp` has three additive Cargo features. The default remains the complete,
backwards-compatible product profile:

| Feature | Owns | Implies |
| --- | --- | --- |
| `decode` | static-image decode, inspection, metadata, and incremental APIs | — |
| `animation` | animation parsing and decode APIs | `decode` |
| `encode` | static-image encoder orchestration and public encode APIs | `decode` |

Animation encoding is available only when both `animation` and `encode` are
selected. `encode` intentionally implies `decode`: the bounded VP8 and VP8L
wire primitives are shared implementation details, not a public encoder-only
dependency. This keeps dependencies directional and avoids duplicating codec
state merely to produce an artificial feature boundary.

## Decoder-only verification

The decoder-only contract is verified with the public API test and a fresh
release build:

```sh
cd webp-rs
cargo test -p webp --no-default-features --features decode --test public_api
CARGO_TARGET_DIR="$(mktemp -d)" \
  cargo check -p webp --no-default-features --features decode
CARGO_TARGET_DIR="$(mktemp -d)" \
  cargo build -p webp --release --no-default-features --features decode
```

The release build's Rust dependency file must not list any encoder
orchestration owner: `static_image/writer`, `animated_image/writer`,
`alpha/plane_writer`, `vp8/frame_writer`, or `vp8l/image_writer`. This checks
the compiled source set rather than inferring it from exported names. The
remaining `BitWriter` and VP8 boolean encoder are private shared wire
primitives used by lossless-alpha decoding and VP8 coefficient parsing; they
are deliberately not part of the public encode API.

## 2026-07-22 local baseline

Fresh, separate release targets on the development machine produced:

| Profile | Cargo-reported build time | `libwebp.rlib` | Production dependencies |
| --- | ---: | ---: | --- |
| default (`decode`, `encode`, `animation`) | 1.99 s | 2,099,432 B | `webp-container` |
| decode-only | 1.10 s | 1,286,752 B | `webp-container` |

The decoder-only archive is 812,680 B (38.7%) smaller in this measurement.
Compilation timing is informational and machine-dependent; the stable
regression contract is the feature-matrix build, public decoder API test, and
the absence of the named writer sources from decoder-only compilation.
