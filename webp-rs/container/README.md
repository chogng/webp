# `webp-container`

This crate owns strict RIFF/WebP container parsing, generic chunk muxing, and
unchanged-file editing. Codec payload parsing belongs to the codec layer, not
to this crate.

## Architecture performance baseline

The following is a recorded local release-build baseline for later container
architecture work. It was measured on 2026-07-22 against main baseline
`7f5cd83c`. It is data, not an executable test or a libwebp comparison. The
corpus contains the 39 `MustAccept` files from `reference-v1` and
`animation-v1`; file I/O and input selection were outside the timed interval.

| Operation | Work per measurement | Three runs (ms) | Median (ms) | Stable result |
| --- | ---: | ---: | ---: | --- |
| Strict demux plus public queries | 390,000 parses; 2,971,380,000 input bytes | 27.631, 26.313, 31.479 | 27.631 | checksum `8020000` |
| Generic mux from owned raw chunks | 39,000 outputs; 297,138,000 output bytes | 21.467, 17.569, 21.773 | 21.467 | checksum `300336000` |
| Strict unchanged editor round trip | 39,000 outputs; 297,138,000 output bytes | 34.337, 28.074, 31.106 | 31.106 | exact input bytes; checksum `300336000` |

Use these figures when considering a container boundary or representation
change. Repeat the same corpus and work definition before claiming a speedup;
the editor's byte identity is a correctness invariant, not merely a checksum.
