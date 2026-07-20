# Fuzzing

The current public-API targets are:

```sh
cargo install cargo-fuzz
rustup toolchain install nightly
cargo +nightly fuzz run container_raw -- -dict=fuzz/dictionaries/webp.dict
cargo +nightly fuzz run incremental_raw -- -dict=fuzz/dictionaries/webp.dict
cargo +nightly fuzz run vp8l_header_raw -- -dict=fuzz/dictionaries/webp.dict
cargo +nightly fuzz run vp8l_raw -- -dict=fuzz/dictionaries/webp.dict
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
used by the public decoder test. The script only adds or refreshes files it
owns; it does not delete findings from a local fuzz corpus.

`container_raw` exercises metadata container parsing, `incremental_raw` varies
chunk boundaries for the public incremental state machine, and
`vp8l_header_raw` reaches VP8L header validation through `read_info`.
`vp8l_raw` wraps its raw input in a `RIFF/WEBP` `VP8L` chunk and reaches the
bounded public `decode` path, including the supported VP8L entropy decoder.
Each uses explicit byte, dimension, metadata, allocation, and work limits. Run
`tools/update-fuzz-dictionary.sh` after refreshing the test-only oracle to copy
the current upstream dictionary into the checked-in fuzz target.

Future targets will cover structured VP8L entropy, animation, mux/demux, and
encode/decode round trips once those public APIs exist.
