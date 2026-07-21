# M5+: completing the WebP product surface

M4 delivers deterministic static VP8L lossless encoding. The following
milestones extend that baseline without weakening its safety or oracle gates.

## M5: static metadata muxing

- Add a public static lossless encoder entry point that accepts ICCP, EXIF, and
  XMP metadata.
- Emit a strict `VP8X` header, correctly declare alpha and metadata flags, and
  preserve chunk ordering and zero padding.
- Verify all metadata combinations through public metadata reads, pixel decode,
  strict container parsing, and the locked libwebp decoder.

**Status: complete.** `encode_lossless_rgba_with_metadata` writes strict
`VP8X`/`ICCP`/`VP8L`/`EXIF`/`XMP` ordering, with exact public metadata and
locked-`dwebp` oracle coverage for every metadata combination. The same raw
metadata contract now applies to the VP8L-frame animation encoder.

## M6: practical VP8L coding tools

- Add bounded LZ77, palette/indexing, adaptive cache sizing, predictor choice,
  color transform choice, and adaptive Huffman construction.
- Preserve exact RGBA output and compare every emitted feature slice with the
  locked oracle. Rate comparisons are informative until a reviewed target is
  set; no heuristic is accepted without deterministic bounds and tests.

**Status: in progress.** Deterministic frequency-ranked balanced Huffman
tables now cover all static VP8L entropy alphabets, including cache references.
The encoder also selects a zero-to-four-bit cache only when it produces hits,
writes color-indexing transforms for deterministic palettes of up to 16
colors, and emits bounded distance-one LZ77 runs (at most 4096 pixels).
It also selects between no predictor and fixed-left prediction from transformed
neighbour matches. Adaptive color-transform choices retain their own exit
criteria.

## M7: static lossy VP8 encoding

- Add an explicit quality/configuration API, RGB(A)-to-YUV conversion,
  intra-mode decision, transform/quantization, coefficient token coding, and
  frame/container output.
- Exit requires locked `dwebp` pixel-oracle coverage plus quality and resource
  measurements across the reviewed encoder option matrix.

**Status: in progress.** The VP8 boolean arithmetic writer and a strict,
locked-`dwebp`-validated DC/zero-residual key-frame payload are implemented.
RGB(A)-to-YUV conversion, quantized coefficients, and the public lossy-image
API remain the next pieces of this milestone.

## M8: animation encoding

- Add bounded canvas/frame APIs, `VP8X`/`ANIM`/`ANMF` serialization, frame
  offsets, duration, blend/dispose, alpha, and static VP8L frame payloads.
- Exit requires locked `webpmux`/`dwebp` vectors and public animation decode
  round trips for all supported frame states.

**Status: complete for the VP8L-frame profile.**
`encode_lossless_animation` serializes bounded VP8L frame rectangles with
alpha, even offsets, durations, blend/dispose flags, canvas color, and loop
count. Strict Rust parse/decode tests cover composition states; the locked
`webpmux` parser and extracted-frame `dwebp` oracle validate emitted files.
Lossy VP8 animation frames naturally remain gated on M7.

## M9: quality-gate closure

- Establish reproducible release benchmarks and profiles for every changed
  public encoder and remaining decoder path.
- Record allocation and CPU hotspots, run external oracle comparisons where
  applicable, and set reviewed regression thresholds.

Each milestone is independently shippable; a later performance or coding-tool
milestone must not silently change the public correctness contract established
by an earlier one.
