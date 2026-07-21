# M4: static VP8L encoding functional exit

M4 starts encoding with one deliberately narrow, independently verifiable
slice: static, lossless VP8L output from straight RGBA8 input. It preserves
the decoder's safety boundary and makes no rate-distortion or throughput claim.

## First-slice public contract

- `webp::encode_lossless_rgba(width, height, rgba)` accepts nonzero VP8L-size
  dimensions and exactly `width * height * 4` straight RGBA8 bytes.
- It returns a complete strict RIFF/WebP file with one `VP8L` chunk. Alpha is
  represented in VP8L itself; the slice emits no `VP8X`, `ALPH`, metadata, or
  animation chunks.
- Invalid dimensions, mismatched input lengths, size overflow, and allocation
  failure are reported through the stable `EncodeError` type.

## Initial coding scope

- Emit VP8L's reversible subtract-green transform, followed by a fixed left
  predictor. Its predictor-mode subimage uses the ordinary nested VP8L image
  syntax. The residual stream uses deterministic frequency-ranked balanced
  canonical Huffman tables.
- Select a bounded zero-to-four-bit VP8L color cache only when a deterministic
  hit-count pass finds a hit, then write cache-reference symbols only for exact
  matches. Repeated images with up to 16 colors may use VP8L color indexing.
  Emit no meta-Huffman groups. Repeated residual runs may use bounded
  distance-one backward references; broader LZ77 search remains M6 work.
- This is intentionally not a compression or performance implementation.
  Palette selection, adaptive cache sizing, adaptive predictor and color
  transforms, LZ77, and encoder tuning remain later M6 work.

## Functional exit criteria

- Direct Rust encode/decode round trips preserve exact dimensions and straight
  RGBA bytes, including transparent pixels. The deterministic matrix covers
  one-pixel axes, odd dimensions, 4×4 predictor boundaries, and opaque plus
  translucent samples.
- The encoder rejects boundary dimensions and RGBA-length mismatches without
  panicking.
- When `third_party/oracle/libwebp` is present, the test verifies that its Git
  HEAD matches `tools/corpus-lock.toml`, decodes Rust-produced output with the
  pinned `dwebp`, and compares its canonical opaque RGBA output byte-for-byte.
  For translucent output it compares every alpha byte and opaque RGB only,
  because `dwebp`'s PAM writer premultiplies translucent RGB.
- All normal workspace tests remain green. Oracle tests remain safely skipped
  only when the pinned local oracle has not been fetched.

## Non-goals

No lossy VP8 encoding, incremental encoding, or encoder performance/size
target is included in this slice. Static metadata muxing and VP8L-frame
animation encoding were added later as M5+ work.

## Exit status

**Complete.** The M4 functional scope is a safe, deterministic static VP8L
encoder that accepts every representable straight-RGBA8 image and emits the
documented valid subset of the VP8L format. Future encoder work may add coding
tools or optimize rate and throughput, but is outside this functional exit.
