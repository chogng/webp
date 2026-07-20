# Fuzzing

The current public-API targets are:

```sh
cargo install cargo-fuzz
cargo fuzz run container_raw -- -dict=fuzz/dictionaries/webp.dict
cargo fuzz run incremental_raw -- -dict=fuzz/dictionaries/webp.dict
cargo fuzz run vp8l_header_raw -- -dict=fuzz/dictionaries/webp.dict
cargo fuzz run vp8l_raw -- -dict=fuzz/dictionaries/webp.dict
```

`container_raw` exercises metadata container parsing, `incremental_raw` varies
chunk boundaries for the public incremental state machine, and
`vp8l_header_raw` reaches VP8L header validation through `read_info`.
`vp8l_raw` wraps its raw input in a `RIFF/WEBP` `VP8L` chunk and reaches the
bounded public `decode` path, including the supported VP8L entropy decoder.
Each uses explicit byte, dimension, metadata, allocation, and work limits. The
dictionary is copied from the locked `libwebp` v1.6.0 reference source.

Future targets will cover structured VP8L entropy, animation, mux/demux, and
encode/decode round trips once those public APIs exist.
