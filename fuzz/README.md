# Fuzzing

The current public-API targets are:

```sh
cargo install cargo-fuzz
rustup toolchain install nightly
cargo +nightly fuzz run container_raw -- -dict=fuzz/dictionaries/webp.dict
cargo +nightly fuzz run incremental_raw -- -dict=fuzz/dictionaries/webp.dict
cargo +nightly fuzz run animation_raw -- -dict=fuzz/dictionaries/webp.dict
cargo +nightly fuzz run vp8l_header_raw -- -dict=fuzz/dictionaries/webp.dict
cargo +nightly fuzz run vp8l_raw -- -dict=fuzz/dictionaries/webp.dict
cargo +nightly fuzz run vp8l_huffman
cargo +nightly fuzz run vp8l_transforms
cargo +nightly fuzz run vp8_bool_raw
cargo +nightly fuzz run vp8_partition_raw
cargo +nightly fuzz run vp8_transforms
cargo +nightly fuzz run vp8_coefficients
cargo +nightly fuzz run vp8_residuals
```

`cargo-fuzz` enables AddressSanitizer with nightly-only Rust compiler options;
the stable toolchain cannot build these targets.

Before the first run, materialize the ignored seed directories from the
committed fixture corpus:

```sh
python3 tools/bootstrap-fuzz-corpus.py
```

This seeds `container_raw`, `incremental_raw`, and `vp8l_header_raw` with every
committed WebP fixture, including structural rejection cases. `vp8l_raw` gets
three deterministic raw bitstream seeds, including the 1x1 literal-only stream
used by the public decoder test. `animation_raw` always gets a minimal valid
`ANIM`/`ANMF` seed and additionally uses the external animation corpus when it
is present. The script only adds or refreshes files it owns; it does not delete
findings from a local fuzz corpus.

`container_raw` exercises metadata container parsing, `incremental_raw` varies
chunk boundaries for the public incremental state machine, and `animation_raw`
drives the complete public animation path through `ANIM`/`ANMF` parsing,
frame decoding, and canvas composition under tight limits.  Then
`vp8l_header_raw` reaches VP8L header validation through `read_info`.
`vp8l_raw` wraps its raw input in a `RIFF/WEBP` `VP8L` chunk and reaches the
bounded public `decode` path, including the supported VP8L entropy decoder.
`vp8l_huffman` keeps the alphabet size valid while mutating the encoded tree.
`vp8l_transforms` generates bounded valid image/configuration shapes and
exercises predictor, subtract-green, color, and color-indexing inverse
transforms directly. `vp8_bool_raw` splits its input into a VP8 boolean-coded
partition and a probability sequence, then drives the bounded boolean decoder
until a semantic EOF or work limit. `vp8_partition_raw` mutates a complete
raw VP8 payload through key-frame parsing, first-partition controls, and token
partition boundary validation. `vp8_transforms` drives arbitrary signed 4×4
coefficient blocks through the scalar VP8 inverse DCT and WHT primitives.
`vp8_coefficients` varies a bounded token partition, coefficient type,
neighbour context, and scan start through the VP8 coefficient entropy decoder.
`vp8_residuals` extends this to all coefficient blocks in one intra macroblock
while varying its top and left non-zero contexts.
Each uses explicit byte, dimension, metadata, allocation, and work limits. Run
`tools/update-fuzz-dictionary.sh` after refreshing the test-only oracle to copy
the current upstream dictionary into the checked-in fuzz target.

Future targets will cover mux/demux and encode/decode round trips once those
public APIs exist.
