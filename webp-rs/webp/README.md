# `webp`

This crate owns the public safe-Rust WebP APIs and orchestrates container,
VP8, VP8L, alpha, and animation behavior.

## SharpYUV architecture performance baseline

This recorded local release-build baseline isolates the private VP8
RGBA-to-YUV420 SharpYUV conversion. It was measured on 2026-07-22 against main
baseline `7f5cd83c`. The 36 static `MustAccept`
`reference-v1` inputs were decoded before timing; each timed run converted
their retained RGBA pixels, excluding decode and file I/O. The checksum hashes
visible Y, U, and V samples only, so macroblock padding cannot affect it.

| Work per measurement | Three runs (ms) | Median (ms) | Stable result |
| ---: | ---: | ---: | --- |
| 720 conversions; 47,185,920 RGBA bytes | 329.111, 343.824, 331.715 | 331.715 | checksum `8846700267572315064` |

Use this as the before-value for future SharpYUV architecture or SIMD work.
The byte-exact pinned-upstream vector test remains the correctness oracle;
this table records speed only.

### Safe SIMD kernel result

The first SIMD pass ports upstream's three 10-bit working-domain kernels:
row filtering, luma error update, and chroma error update. It uses 8-lane
`wide` vectors, retains scalar tails and non-SIMD fallbacks, and keeps the
workspace's `unsafe_code = "forbid"` policy. Five forward/reverse alternating
release runs compared binaries built from scalar `main@ffa826349091` and the
candidate on the same arm64 host:

| Scope | Scalar runs (ms) | SIMD runs (ms) | Median change |
| --- | ---: | ---: | ---: |
| SharpYUV, 1,800 conversions / 117,964,800 RGBA bytes | 930.744, 839.921, 844.637, 877.546, 837.398 | 665.756, 665.103, 672.763, 679.439, 667.634 | 844.637 -> 667.634 (**-20.96%**) |
| VP8 encode, 630 encodes / 41,287,680 RGBA bytes | 943.089, 970.248, 965.340, 1277.131, 971.501 | 888.321, 890.904, 907.759, 903.376, 918.909 | 970.248 -> 903.376 (**-6.89%**) |

All direct runs retain checksum `3670006595221236044`; all VP8 runs retain
output bytes `1,901,740`, checksum `1,953,400`, and the established per-quality
size, SSE, and PSNR values. The direct benchmark binary grows by 320 bytes
(0.048%). The benchmark also feeds one Rust-decoded RGBA corpus to pinned
libsharpyuv with and without its native dispatch; on this host and small corpus,
the C SIMD and scalar medians are 1087.540 and 1087.498 ms respectively,
effectively flat. This diagnostic is kept separate from the Rust scalar/SIMD
product decision.

Reproduce the direct Rust and pinned-C comparison with:

```sh
bash tools/benchmark-sharp-yuv.sh 20
```
