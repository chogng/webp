# M1/M2 feature matrix

| Feature | Unit tests | Public-path tests | Fuzz target |
| --- | --- | --- | --- |
| bit reader | private `webp::bit_io` owner | `webp::read_info`, `webp::decode` | `vp8l_raw`, `vp8l_huffman` |
| checked arithmetic | private `webp::limits` and independent `webp-container` arithmetic | bounded `read_info`/`decode`/incremental paths | all public raw targets |
| RIFF | `webp-container::parser` | direct fixture tests and all-prefix truncation | `container_raw`, `incremental_raw` |
| VP8X | `webp-container::vp8x` | `webp::read_info`, strict decode validation | `container_raw` |
| VP8L header | private `webp::vp8l` header boundary suite | `read_info`, upstream lossless corpus | `vp8l_header_raw`, `vp8l_raw` |
| VP8L Huffman | balanced/unbalanced/repeat/tree rejection suites | 41 upstream lossless RGBA goldens | `vp8l_huffman`, `vp8l_raw` |
| VP8L LZ77 | prefix ranges and slow-copy differential | upstream lossless RGBA goldens | `vp8l_raw` |
| VP8L transforms | predictor/color/indexing unit suites | all 16 transform combinations and payload-prefix truncation | `vp8l_transforms`, `vp8l_raw` |
| VP8 frame header | private `webp::vp8` tag, signature, dimension, canvas, and truncation suites | `webp::read_info` on an unextended VP8 RIFF | `vp8_partition_raw` |
| VP8 boolean entropy | private `webp::vp8` probability-boundary, literal, EOF, budget, and arithmetic-vector suites | first-partition parser | `vp8_bool_raw` |
| VP8 partition layout | private `webp::vp8` segmentation/filter controls, 1/2/4/8 layout, and table-boundary suites | VP8 decode path | `vp8_partition_raw` |
| VP8 quantization | base-index/delta vectors and dequant clamp/segment matrices | coefficient reconstruction (pending) | `vp8_partition_raw` |
| VP8 coefficient probabilities | RFC table samples and all-flag default parsing | token decode (pending reconstruction) | `vp8_partition_raw` |
| VP8 inverse transforms | 4×4 DCT and WHT zero/DC microvectors | coefficient reconstruction (pending token decode) | transform target (pending next M2 slice) |
