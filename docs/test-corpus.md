# WebP test corpus contract

The test corpus is deliberately split into five sources. Fixture expectations
live beside the public API tests that consume them; downloaded data never
silently becomes a release artifact.

| Data class | Location | PR | Nightly | Release |
| --- | --- | --- | --- | --- |
| Upstream conformance vectors | `third_party/corpus/libwebp-test-data` | 68-file selected list | pinned selected vectors | supported vectors × public API matrix |
| libwebp reference output | `third_party/oracle/libwebp` and generated outputs | selected supported features | pairwise encoder set and differential output | all supported encoder options |
| Real-image benchmark | `third_party/benchdata/clic` | none | CLIC validation | CLIC validation plus approved larger splits |
| Structured hostile input | ignored local fixture cache | deterministic malformed cases | generated matrix plus fuzz corpus | all cases under normal and tight limits |
| Historical regressions | `tests/fixtures/regressions` | all | all | all |

## Pinned upstream inputs

`tools/corpus-lock.toml` names the allowed upstream URLs and immutable commits.
The `tracking_branch` is used only by the manual update scripts; ordinary CI
and release builds fetch the reviewed commit. Fetch only through:

```text
tools/fetch-libwebp-test-data.sh
tools/fetch-libwebp-oracle.sh
tools/verify-upstream-smoke.sh
```

Each invocation checks out the pinned commit and verifies it after checkout.
`tests/corpora/libwebp-test-data-smoke-v1.sha256` records the selected vector
hashes for that commit, so a changed upstream file is visible in review. The
repository does not automatically track upstream changes: maintainers check
and advance the pin deliberately, never changing the data consumed by normal
CI in place.

When an update is wanted, first check whether the tracking branch has advanced:

```sh
sh tools/check-libwebp-test-data-update.sh
```

Then make the change deliberately:

```sh
sh tools/update-libwebp-test-data-lock.sh
tools/fetch-libwebp-test-data.sh
sh tools/update-upstream-smoke-lock.sh
tools/verify-upstream-smoke.sh
```

Review and commit the updated pin and checksum lock only after the external
corpus test passes.

`webp-rs/decode/tests/external_upstream_corpus.rs` reads the versioned smoke
selection directly. It promotes only the VP8L vectors supported by the public
decoder; additional vectors are added to direct API tests when their codec path
is implemented.

The reference checkout is test-only. It supplies the upstream fuzz dictionary,
future `cwebp` pairwise encoder vectors, `webpmux` metadata vectors, and
animation oracle outputs. It is not linked into the published Rust codec.

Build `cwebp` from that checkout, then run
`tools/generate-reference-corpus.sh`. It produces the 36-vector quality/method
matrix outside Git for direct Rust API tests.

Run `python3 tools/generate-reference-edge-corpus.py` to add the separate
66-vector RGB/RGBA edge matrix: 1×1, odd dimensions, a long row, alpha, lossy,
lossless, and near-lossless settings for later direct pixel-golden tests.

## Generated fixtures

Run `cd webp-rs && cargo run -p xtask -- fixtures ensure` once after cloning.
It computes the expected deterministic set in memory and validates the ignored
`tests/fixtures/generated/` cache. A matching cache is a zero-write hit; a
missing, stale, incomplete, or corrupt cache is rebuilt under a cross-process
lock. `fixtures verify` is the read-only integrity check.

Each immutable generation is addressed by the SHA-256 of a canonical manifest.
The manifest records the complete relative path, size, and SHA-256 of every
minimal RIFF/VP8X hostile input and metadata-matrix file. A generation is built
and synced in a sibling staging directory, renamed into `sets/<digest>`, and
only then made current through a monotonically numbered marker. Readers
therefore select either a complete old generation or a complete new one; they
never infer coverage by listing an in-progress directory. Tests and the fuzz
bootstrap consume the same manifest.

`fixtures generate` remains only as a deprecated compatibility alias for
`ensure`. Normal test runs read the committed generation marker and never
regenerate it.
A discovered failure is minimized before being moved to
`tests/fixtures/regressions/`, with its issue/source, expected result, and the
API path that previously failed.

Use `tools/promote-regression.sh <input.webp> <id> <issue-or-source> <license>`
to copy a fixture, then add its direct public API test. Rejection regressions
should cover one-shot, `ReadInfo`, and incremental decoding; accepted-image
regressions should assert their relevant dimensions or pixel goldens.

Animation vectors must be generated from libwebp tools, retaining the resolved
oracle commit, raw RGBA input, WebP output, per-frame composed RGBA hashes,
rectangles, duration, blend/dispose flags, loop count, background color,
oracle revision, and generator arguments.

The cached deterministic metadata matrix covers ICCP, EXIF, XMP, their
combinations, boundary payload lengths, legal chunk positions, and padding
without committing one binary file per Cartesian-product case. Malformed
metadata layouts, duplicates, incorrect declared sizes, and truncations use the
same ignored cache.

`tools/generate-animation-corpus.sh` creates the initial two-frame loop. The
animation test is deliberately separate from pixel decode until frame
composition is exposed by the public API.

`python3 tools/generate-animation-state-corpus.py` adds blend/dispose, offset,
duration, loop-count, and background-color container states using `webpmux`.

For fuzzing, run `python3 tools/bootstrap-fuzz-corpus.py` to materialize the
ignored target-specific seed directories from the committed fixtures. It also
provides minimal raw VP8L entropy seeds; findings are minimized and promoted
through `tools/promote-regression.sh`, not committed directly from a fuzzer.

## Execution profiles

PR tests only assert features already implemented by the public API. A valid
upstream image is not labelled `MustAccept` until its pixels can be checked.
Nightly/release jobs expand only after the corresponding decoder/encoder API
exists: one-shot versus incremental, RGBA/BGRA/RGB outputs, scalar/SIMD, and
normal versus tight resource limits must agree.

CLIC is strictly a rate-distortion/performance corpus: output size, encode and
decode time, PSNR/Y-PSNR, SSIM/MS-SSIM, peak memory, and thread scaling. It is
not a bitstream-conformance or pixel-golden corpus. The source registry intentionally
starts with the validation split; download it outside Git only when the encoder
benchmark harness lands.

TFDS must download the whole 7.48 GiB CLIC archive (about 14.96 GiB of
workspace after preparation) before exposing validation. To make that explicit,
fetch and normalize it with
`python3 tools/fetch-clic-validation.py --allow-full-download`. It writes
ignored PNG inputs plus a SHA-256/geometry manifest for the Rust benchmark
harness; no CLIC image enters the release crate or conformance fixture set.

When the two official validation zips have already been downloaded, avoid TFDS
entirely with `--mobile-zip /path/mobile_valid_2020.zip --professional-zip
/path/professional_valid_2020.zip`. The exporter keeps the archive SHA-256
values in `validation-manifest.json` alongside per-image integrity data. Once
imported, runtime benchmarks need only `validation-png/` and this manifest;
the source zips may be archived or deleted. Run
`python3 tools/verify-clic-validation.py` to recheck the local corpus without
the source zips.
