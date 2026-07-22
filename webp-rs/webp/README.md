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
