# Invalid screen: stale Rust test filter

The first P20 screen attempt used the unchanged final product binary
`9aa8fa08fb2288335a98ff4a3d5f64a5a7922372f7c4c1b1212ed98b9b1a29f8`
and wrote external output to
`/private/tmp/vp8l-profile-hybrid-product-p20-screen-final`.

The shared `tools/run-vp8l-product-benchmark.py` still selected
`encoder::product_benchmark_tests::product_validation_reproducer`, while the
actual final-binary test is
`vp8l::image_writer::product_benchmark_tests::product_validation_reproducer`.
Every Rust encode and Rust decode child therefore matched zero tests, exited
zero, and emitted a 115-byte test summary with no measurement or aggregate
row. The external summarizer correctly failed when no aggregate existed.

The six-layout generator, pinned-libwebp comparison, and 36 pinned-C decode
processes completed with empty stderr, but they cannot form a fair screen
without the paired Rust measurements. No timing or gate claim from this run is
valid. The runner is corrected to the actual filter and now rejects any child
without an aggregate row. The complete 41-image screen restarts from warmup in
a new output directory; the Rust product binary is unchanged.
