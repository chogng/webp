# Fuzzing

M0 starts with the raw container safety target:

```sh
cargo install cargo-fuzz
cargo fuzz run container_raw
```

The target calls the public metadata path under explicit byte, dimension, and
metadata limits. It must never panic or exceed its deterministic limits. VP8L
raw, Huffman-structured, and incremental targets are M1 work.

