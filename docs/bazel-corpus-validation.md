# Bazel and external-corpus validation

This document describes what the repository's Bazel tests, dependency locks,
and external WebP corpus checks cover.

## Routine CI checks

The `test` job runs, in order:

1. **Rust baseline.** Rust 1.88.0 runs formatting, Clippy, and
   `cargo test --workspace`. This matches the Bazel Rust toolchain.
2. **Bzlmod lock.** `tools/check-module-bazel-lock.sh` runs
   `bazel mod deps --lockfile_mode=error`. CI fails if
   `MODULE.bazel.lock` is stale; it never rewrites the lock silently.
3. **Bazel test graph.** `bazel test --test_output=errors
   --test_tag_filters=-external-corpus //...` builds and runs ordinary
   `rust_test` targets. Committed fixtures enter the sandbox through Bazel
   `data` declarations, so tests cannot rely on undeclared machine-local files.
4. **Cargo-to-Bazel dependency mapping.** CI builds with
   `CARGO_BAZEL_REPIN=1` and then uses `git diff --exit-code --
   cargo-bazel-lock.json` to detect an uncommitted generated lock update.

External-corpus targets carry the `manual` and `external-corpus` tags.
`third_party/` is Git-ignored and contains only downloaded test data, the
libwebp oracle, and benchmarks. Therefore, an absent external corpus cannot be
mistaken for a successful corpus validation.

## Upstream corpus checks

`tools/corpus-lock.toml` records an immutable commit for
`libwebp-test-data`. The fetch script checks out only that commit and verifies
the resulting `HEAD`.

`tests/corpora/libwebp-test-data-smoke-v1.sha256` records the paths and
SHA-256 values of the 68 selected vectors.
`tools/verify-upstream-smoke.sh` verifies:

1. the expected Git checkout;
2. its commit matches the checksum-lock revision;
3. all 68 selected paths exist;
4. the checksum lock has exactly 68 entries; and
5. each file matches its recorded SHA-256.

Scheduled `upstream-corpus` CI uses this pinned input and explicitly runs
`//crates/webp:external_upstream_corpus_test`. The test reads the corpus from
Bazel runfiles and fails if the corpus was not supplied.

## Manual updates

The repository does not poll upstream automatically or create reminder issues.
When a maintainer chooses to check for a newer corpus, run:

```sh
sh tools/check-libwebp-test-data-update.sh
```

If an update is desired, run:

```sh
sh tools/update-libwebp-test-data-lock.sh
tools/fetch-libwebp-test-data.sh
sh tools/update-upstream-smoke-lock.sh
tools/verify-upstream-smoke.sh
```

Review and commit the resulting changes to `corpus-lock.toml` and the checksum
lock.

## Validation performed for this update

- `cargo test --workspace` passed.
- Modified shell scripts passed `sh -n` syntax checks.
- Pinned commit `06ddd96e276c2c638a72d39d3c0f340afd61978c` downloaded
  successfully.
- All 68 selected vectors passed path and SHA-256 verification.
- The upstream `main` branch pointed at the same commit when checked, so no
  update was required.
- `git diff --check` passed.
