# Reconstructed invalidated validation audit

This record is reconstructed from the task tool output. It is **not** the
original raw log. The original raw file was overwritten after a later path
mistake described below. This file preserves that fact rather than presenting
the reconstruction as an untouched artifact.

## Incomplete-subtree archive validation

- Command: `CARGO_TARGET_DIR=/private/tmp/vp8l-packed-writer-product-9435fbd0/validation-target cargo test --workspace --all-targets`
- Working directory: `/private/tmp/vp8l-packed-writer-product-9435fbd0/product`
- Source shape: a `git archive` invoked from the repository's `webp-rs/`
  subdirectory, so the workspace sources were exact but repository-root
  `tests/` and other siblings were absent.
- Result: invalidated environment failure before tests ran. It is not a code
  failure and supplies no quality-gate evidence.

The complete combined stdout/stderr captured in the task tool output was:

```text
   Compiling webp-core v0.1.0 (/private/tmp/vp8l-packed-writer-product-9435fbd0/product/core)
   Compiling webp-vp8l-transform v0.1.0 (/private/tmp/vp8l-packed-writer-product-9435fbd0/product/vp8l-transform)
   Compiling webp-vp8l-color-transform v0.1.0 (/private/tmp/vp8l-packed-writer-product-9435fbd0/product/vp8l-color-transform)
   Compiling serde_core v1.0.229
   Compiling serde v1.0.229
   Compiling hashbrown v0.17.1
   Compiling equivalent v1.0.2
   Compiling toml_write v0.1.2
   Compiling winnow v0.7.15
   Compiling webp-vp8l-entropy v0.1.0 (/private/tmp/vp8l-packed-writer-product-9435fbd0/product/vp8l-entropy)
   Compiling webp-vp8l v0.1.0 (/private/tmp/vp8l-packed-writer-product-9435fbd0/product/vp8l)
   Compiling webp-vp8l-huffman v0.1.0 (/private/tmp/vp8l-packed-writer-product-9435fbd0/product/vp8l-huffman)
   Compiling webp-vp8l-color-cache v0.1.0 (/private/tmp/vp8l-packed-writer-product-9435fbd0/product/vp8l-color-cache)
   Compiling webp-vp8l-indexing v0.1.0 (/private/tmp/vp8l-packed-writer-product-9435fbd0/product/vp8l-indexing)
   Compiling webp-animation v0.1.0 (/private/tmp/vp8l-packed-writer-product-9435fbd0/product/animation)
   Compiling webp-vp8 v0.1.0 (/private/tmp/vp8l-packed-writer-product-9435fbd0/product/vp8)
   Compiling webp-container v0.1.0 (/private/tmp/vp8l-packed-writer-product-9435fbd0/product/container)
   Compiling webp-vp8l-literal v0.1.0 (/private/tmp/vp8l-packed-writer-product-9435fbd0/product/vp8l-literal)
   Compiling indexmap v2.14.0
   Compiling webp-alpha v0.1.0 (/private/tmp/vp8l-packed-writer-product-9435fbd0/product/alpha)
   Compiling webp v0.1.0 (/private/tmp/vp8l-packed-writer-product-9435fbd0/product/webp)
error: couldn't read `webp/tests/../../../tests/corpora/libwebp-test-data-smoke-v1.txt`: No such file or directory (os error 2)
  --> webp/tests/external_upstream_corpus.rs:13:5
   |
13 |     include_str!("../../../tests/corpora/libwebp-test-data-smoke-v1.txt");
   |     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

error: could not compile `webp` (test "external_upstream_corpus") due to 1 previous error
warning: build failed, waiting for other jobs to finish...
```

## Failed archival move and overwrite

Before the full-archive validation retry, the task attempted to preserve the
directory with this relative command:

```text
mv experiments/vp8l-packed-writer-product/raw/validation experiments/vp8l-packed-writer-product/raw/invalidated-validation-subtree-archive
```

The active working directory was
`/private/tmp/vp8l-packed-writer-product-9435fbd0/product-full/webp-rs`, not the
repository root, so the move missed its target and emitted exactly:

```text
mv: rename experiments/vp8l-packed-writer-product/raw/validation to experiments/vp8l-packed-writer-product/raw/invalidated-validation-subtree-archive: No such file or directory
```

The subsequent retry used absolute output paths. It therefore overwrote the
original `raw/validation/test-debug.log` and `validation.tsv` names with the
successful full-archive validation artifacts. The original raw failure log no
longer exists. The successful replacement files are genuine raw output; this
reconstructed Markdown file is the explicit audit substitute for the lost
failure log.

The final quality gate ran from the complete archive at
`/private/tmp/vp8l-packed-writer-product-9435fbd0/product-full/webp-rs` and is
recorded separately in `raw/validation/`.
