# Invalidated validation output root

The first seven validation commands all exited successfully, but they were
launched with cwd `webp-rs` while their relative log paths began with
`experiments/`. They therefore wrote under the stray
`webp-rs/experiments/...` tree instead of the fixed top-level experiment
directory. The logs and the premature status table are retained here exactly
as a superseded run. The empty stray directory hierarchy was removed after its
contents were moved here.

The final validation is rerun from the repository root with
`--manifest-path webp-rs/Cargo.toml` and writes to the correct top-level
`experiments/vp8l-entropy-aware-spatial-clustering/raw/validation-final`.
