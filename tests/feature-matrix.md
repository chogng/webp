# M1/M2 feature matrix

| Feature | Unit tests | Public-path tests | Fuzz target |
| --- | --- | --- | --- |
| bit reader | `webp-core::bit_io` | `webp::read_info`, `webp::decode` | `vp8l_raw`, `vp8l_huffman` |
| checked arithmetic | `webp-core::limits` | bounded `read_info`/`decode`/incremental paths | all public raw targets |
| RIFF | `webp-container::parser` | fixture manifests and all-prefix truncation | `container_raw`, `incremental_raw` |
| VP8X | `webp-container::vp8x` | `webp::read_info`, strict decode validation | `container_raw` |
| VP8L header | `webp-vp8l` header boundary suite | `read_info`, upstream lossless corpus | `vp8l_header_raw`, `vp8l_raw` |
| VP8L Huffman | balanced/unbalanced/repeat/tree rejection suites | 41 upstream lossless RGBA goldens | `vp8l_huffman`, `vp8l_raw` |
| VP8L LZ77 | prefix ranges and slow-copy differential | upstream lossless RGBA goldens | `vp8l_raw` |
| VP8L transforms | predictor/color/indexing unit suites | all 16 transform combinations and payload-prefix truncation | `vp8l_transforms`, `vp8l_raw` |
| VP8 frame header | `webp-vp8` tag, signature, dimension, canvas, and truncation suites | `webp::read_info` on an unextended VP8 RIFF | VP8 raw target (pending next M2 slice) |
| VP8 boolean entropy | `webp-vp8` probability-boundary, literal, EOF, budget, and arithmetic-vector suites | first-partition parser (pending next M2 slice) | `vp8_bool_raw` |
