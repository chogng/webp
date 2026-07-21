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

**Status: complete.** Deterministic frequency-ranked balanced Huffman
tables now cover all static VP8L entropy alphabets, including cache references.
The encoder also selects a zero-to-four-bit cache only when it produces hits,
writes color-indexing transforms for deterministic palettes of up to 16
colors, and emits bounded distance-one LZ77 runs (at most 4096 pixels).
It also selects between no predictor and fixed-left prediction from transformed
neighbour matches. For images of at least 256 pixels, it also evaluates a
bounded set of global color-transform coefficient triples and emits one only
when its signed channel-residual score improves by at least 25%; nested-table
cost otherwise keeps the color transform disabled. Per-block coefficient
selection remains a later rate-tuning step.

Each emitted coding-tool profile is validated through public Rust round trips
and the locked `dwebp` oracle: color indexing, non-palette cache hits,
non-palette left prediction with bounded distance-one LZ77, adaptive Huffman
tables, and the global color transform. A release one-pass rate record is
captured in the quality gates; cross-encoder thresholds remain M9 work.

## M7: static lossy VP8 encoding

- Add an explicit quality/configuration API, RGB(A)-to-YUV conversion,
  intra-mode decision, transform/quantization, coefficient token coding, and
  frame/container output.
- Exit requires locked `dwebp` pixel-oracle coverage plus quality and resource
  measurements across the reviewed encoder option matrix.

**Status: complete for the bounded static intra16 profile.** The public API
accepts explicit quality 0 through 100, converts straight RGBA8 to padded
YUV420, preserves alpha through a strict raw `ALPH` container path, and emits
one-token-partition key frames. Every macroblock evaluates DC, vertical,
horizontal, and true-motion luma/chroma prediction, then deterministically
ranks reconstructed distortion and quantized coefficient cost. It uses the
same reconstructed-neighbour borders as the decoder, including at visible
edges. RGB(A), opaque multi-macroblock, alpha, and quality-matrix outputs are
validated by the locked `dwebp` pixel oracle. The release matrix benchmark and
resource notes are recorded in the quality gates; cross-encoder thresholds
remain M9 work.

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

**Status: in progress.** VP8 encoder profiling removed redundant cross-product
work and plane reconstruction from intra16 mode search while preserving
bit-for-bit output, improving the locked quality matrix by 61.6% in total.
The pinned libwebp comparison shows Rust 20.3% faster but producing 2.13x as
many bytes. Zero-residual macroblock skipping now uses an adaptive probability
and an exact no-expansion decision. Frame-adaptive coefficient probabilities
then reduce the locked matrix from 288,010 to 183,802 bytes (36.18%) and narrow
the libwebp size ratio from 2.13x to 1.359x, at the cost of moving Rust from
20.3% faster to 13.1% slower on the same comparison. VP8 encoder work therefore
shifts to an explicit rate/distortion gate and recovery of candidate-encoding
CPU cost. Reviewed regression thresholds and the remaining public paths are
still open.

Each milestone is independently shippable; a later performance or coding-tool
milestone must not silently change the public correctness contract established
by an earlier one.
