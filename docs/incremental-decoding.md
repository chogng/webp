# Incremental static-image decoding

`webp::IncrementalDecoder` consumes one RIFF stream through `push` and returns
the final `Image` from `finish`. It is a persistent state machine rather than a
prefix retry wrapper: RIFF and chunk headers are scanned once, and lossy VP8
retains its boolean arithmetic decoder, token-partition cursors, macroblock
neighbour context, loop-filter state, YUV planes, and RGBA output prefix.

`info()` becomes available after the fixed VP8 or VP8L header. `decoded()`
borrows only stable, row-aligned RGBA output. Its slice length is always
`width * decoded_rows * 4`; unreported rows can still be changed by VP8 loop
filtering or fancy 4:2:0 upsampling. A `Progress::DecodedRows` result indicates
that the stable prefix grew, while `Progress::Complete` means the complete RIFF
layout was validated and `finish` can return the image.

The input uses append semantics: each byte belongs to exactly one `push` call.
Empty pushes are allowed before a terminal result. Pushes after completion or
failure return `InvalidParameter`. Calling `finish` before the declared RIFF
body and static codec complete returns `UnexpectedEof`, even if some rows were
already available.

## Codec boundaries in this version

- VP8 is genuinely suspendable inside coefficient token partitions. A short
  input rolls back only the current macroblock, following libwebp's saved
  macroblock-context boundary. Partition 0 is intentionally required in full
  before row decoding begins because it owns immutable frame probabilities.
- Raw and compressed `ALPH` retain their existing decode semantics. Since an
  ALPH chunk precedes its VP8 chunk, the complete alpha plane is decoded before
  VP8 rows are published and applied as each RGBA prefix grows.
- VP8L preserves all existing transforms, meta-Huffman, cache, and LZ77
  semantics, but its first streaming boundary is currently the complete VP8L
  RIFF chunk. It is decoded once during `push`; it is not retried from the
  beginning for each input fragment. Making VP8L entropy commands and inverse
  transforms independently suspendable is the remaining codec-level extension.
- Animated containers are rejected with `UnsupportedFeature`; callers must use
  the separate animation API after collecting a complete container.

All input, dimension, pixel, metadata, allocation, and deterministic work
limits remain active. Input byte limits are enforced before each append.
