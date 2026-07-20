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

`webp::read_info` now obtains dimensions from an unextended `VP8 ` chunk. The
public `decode` path invokes the same validation and then returns
`UnsupportedFeature` until the macroblock, transform, loop-filter, and YUV
stages are implemented. This avoids treating a header-only parse as a pixel
decode.

The next M2 slice is first-partition header parsing, including segmentation,
quantization, and token-partition boundaries.
