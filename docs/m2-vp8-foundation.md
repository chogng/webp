# M2: VP8 decoder foundation

M2 begins with a separately testable, allocation-free VP8 key-frame parser in
`webp-vp8`. It validates the three-byte frame tag, the WebP-legal key-frame
profile and visibility bit, the key-frame start code, dimensions and scale
bits, the first-partition boundary, configured limits, and any enclosing VP8X
canvas.

The same crate now supplies a bounded MSB-first VP8 boolean decoder. It covers
probabilities `0..=255`, fixed-width literals, byte-exact EOF reporting, and a
deterministic work budget. `vp8_bool_raw` fuzzes the decoder directly with a
mutated partition and probability sequence.

The first-partition parser now recovers colour-space/clamp flags, segmentation
and loop-filter controls, and safely splits the coefficient stream into 1, 2,
4, or 8 token partitions. `vp8_partition_raw` fuzzes this complete structural
path, including every size-table boundary.

Quantizer parsing now recovers the base index and all five signed deltas. The
remaining first-partition entropy data is consumed using the canonical 4 × 8 ×
3 × 11 coefficient-probability defaults and update probabilities from RFC
6386; the resulting table and skip probability are retained for token decode.

The parser also derives four scalar dequantization matrices. It applies VP8's
segment inheritance/absolute modes, the distinct UV DC clamp, and the Y2 AC
minimum, so later coefficient reconstruction receives specification-scale
multipliers rather than raw quantizer indices.

Scalar inverse 4×4 DCT and inverse Walsh-Hadamard primitives now provide the
exact fixed-point rounding used by VP8 reconstruction. They return signed
residues only; prediction, clipping, and pixel-plane ownership remain separate
to keep transform failures locally diagnosable.

`webp::read_info` now obtains dimensions from an unextended `VP8 ` chunk. The
public `decode` path invokes the same validation and then returns
`UnsupportedFeature` until the macroblock, transform, loop-filter, and YUV
stages are implemented. This avoids treating a header-only parse as a pixel
decode.

The next M2 slice adds coefficient-token decoding, then connects coefficients
and transforms to macroblock prediction and reconstruction.

Before M2 is marked complete, it must satisfy the milestone exit gates in
[`quality-gates.md`](quality-gates.md), including a release benchmark and
profile for the public VP8 decode path.
