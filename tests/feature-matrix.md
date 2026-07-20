# M0 feature matrix

| Feature | Unit tests | Public-path tests | Fuzz target |
| --- | --- | --- | --- |
| bit reader | `webp-core::bit_io` | fixture runner entry point | pending M0 skeleton |
| checked arithmetic | `webp-core::limits` | `webp::read_info` | pending M0 skeleton |
| RIFF | `webp-container::parser` | smoke fixture manifest | `container_raw` skeleton |
| VP8X | `webp-container::vp8x` | `webp::read_info` | `container_raw` skeleton |
| VP8L header | `webp-vp8l` header boundary suite | `webp::read_info` simple VP8L | `vp8l_raw` pending M1 wiring |
| VP8L Huffman | balanced/unbalanced/tree rejection suite | pending entropy-stream decoder | `vp8l_huffman` pending M1 wiring |
| VP8L LZ77 | prefix ranges and slow-copy differential | pending entropy-stream decoder | `vp8l_lz77` pending M1 wiring |
| VP8L predictor/subtract-green | all modes, borders and channel extremes | pending lossless decoder | `vp8l_transforms` pending M1 wiring |
