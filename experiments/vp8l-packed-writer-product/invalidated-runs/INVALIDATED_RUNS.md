# Invalidated run inventory

None of the artifacts in this inventory contributes to the product headline.
They are retained to make evidence discipline auditable.

## Wrong build root before any benchmark

Three intended archive builds set different `CARGO_TARGET_DIR` values but ran
from the current worktree's `webp-rs/` directory. All three therefore compiled
the same current product sources. The identical, invalid test-binary SHA-256
was `c67f50a809571ed74fbbcc0d99932c3dd562cc56968cef7b7cf04c4b2940ab1e`;
the identical rlib SHA-256 was
`171486a396398f9cab20c450c6856d713be35350c7c236fce906a5da5f80bd98`.
Benchmarking had not started. These hashes and products were discarded.

## Screen preparation non-run

A shell loop assigned to zsh's special `path` variable, replacing `PATH` in
that subprocess. `basename`, `ln`, and `python3` were consequently not found.
No lock was acquired, no runner output directory was created, and no timing
sample exists. Later shell variables use the task-specific `vp8l_*` prefix.

## Pre-manifest partial screen

The first runner attempt began after checking 41 symlinks but before persisting
the requested manifest/hash. It was terminated, the lock was cleaned up, and
its 17 files (warmups plus round-one fragments) were moved intact to
`raw/invalidated-screen-pre-manifest/`. They are not summarized.

## Subtree-archive binaries

Exact commit sources were archived from inside `webp-rs/`, producing workspace
subtrees rather than complete repository archives. Source blob checks passed,
but the binary path provenance did not satisfy the stricter full-archive
discipline. The following binary SHA-256 values and every result produced by
them are invalidated:

- product: `04f3160e1a72a88fee8a7bd88cb124bfd9b4039d728a66b4869e23935c1b3622`;
- latest-main: `b45744af3044bb197d93133be2f50c66106bc2380d52f6697dc5975523678c0f`;
- E36: `bee353f38e3b6f6b129148ff43be062b611ec9ad78d121044c02f49638c095f0`.

Their raw artifacts remain under:

- `raw/invalidated-screen-subtree-archive-binary/`;
- `raw/invalidated-formal-subtree-archive-binary/`;
- `raw/invalidated-identity-latest-main-product-subtree-binaries/`;
- `raw/invalidated-identity-product-e36-subtree-binaries/`.

The final headline uses only the full-archive product binary
`247305b53187841383afb7a39a872f1292728e7a114b0d5541547b101da524fe`.
