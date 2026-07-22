# VP8 SharpYUV contract

The VP8 encoder has one RGB-to-YUV420 production path: the private scalar
SharpYUV implementation in `webp::vp8`. The previous 2x2 box sampler is not
retained as a compatibility profile, fallback, or hidden option. Public lossy
options therefore continue to describe encoder quality only; they do not expose
two competing color-conversion states.

## Owned behavior

- Input is straight RGBA8. RGB is converted independently of alpha; the static
  image writer continues to serialize non-opaque alpha through `ALPH`.
- Sampling uses the sRGB transfer function, WebP's limited-range conversion
  matrix, and four reconstruction-aware refinement iterations.
- Odd visible edges are replicated to the even SharpYUV working geometry.
  `yuv_image` separately owns replication from the visible planes to complete
  VP8 macroblocks.
- The implementation is private to VP8 because it currently has one caller,
  one release lifecycle, and no independent dependency or version boundary. A
  separate crate would add packaging without creating an architectural owner.

The implementation deliberately covers the format path the encoder consumes:
8-bit RGB input and 8-bit WebP YUV output. It does not claim libsharpyuv's
general high-bit-depth, transfer-function, or matrix API.

## Oracle and product validation

`sharp_yuv_tests.rs` locks a high-chroma odd-size vector byte-for-byte against
libsharpyuv commit `733c91e461c18cf1127c9ed0a80dccbcfed599d3`, the immutable
revision in `tools/corpus-lock.toml`. Separate tests cover neutral chroma and
macroblock edge replication. Public VP8 integration tests then decode Rust
outputs with the pinned `dwebp` oracle at quality 0, 75, and 100, including
alpha and multi-macroblock images.

On the 21-file `reference-v1` matrix, the SharpYUV path produces 190,174 bytes
per quality matrix with checksum `195340`. RGB PSNR is 25.860, 37.389, and
48.277 dB at quality 0, 75, and 100. Compared with the preceding box-sampled
baseline, q0 and q75 improve by 0.003 and 0.013 dB, while q100 falls by 0.373
dB; output grows by 3.47%. This is accepted as an explicit color-sampling
product change, not described as a universal PSNR improvement. SharpYUV's
purpose is to optimize chroma for reconstructed 4:2:0 edges and match the
pinned upstream algorithm; aggregate RGB PSNR is retained as a regression
guard rather than its defining objective.

Three five-iteration runs measured 473.650, 477.949, and 478.849 ms; the median
is 477.949 ms. The preceding scalar box path measured 353.658 ms, so the four
reconstruction-aware iterations cost 35.1% on this matrix. The reviewed gate
in `quality-gates.md` allows 5% headroom over the new median. SIMD remains a
future optimization and must preserve the scalar oracle exactly.

Reproduce the complete encode, size, distortion, and pinned-libwebp comparison:

```sh
bash tools/benchmark-vp8-encode.sh 5
```
