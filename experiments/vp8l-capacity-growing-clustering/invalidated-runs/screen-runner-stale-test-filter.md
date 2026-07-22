# Invalidated screen run: stale test filter

The first 41-image encode screen attempt is retained at
`raw/screen-41-encode` and is not rate or performance evidence.

- Every child exited successfully but printed `running 0 tests`.
- `tools/run-vp8l-product-benchmark.py` still selected the historical
  `encoder::product_benchmark_tests::product_validation_reproducer` path.
- The same binary lists the active test as
  `vp8l::image_writer::product_benchmark_tests::product_validation_reproducer`.
- The runner filter was corrected before the replacement screen. No P16 rate or
  clustering rule changed.

