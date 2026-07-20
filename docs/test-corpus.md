# WebP test corpus contract

The test corpus is deliberately split into five sources. A fixture's source,
license, SHA-256, expected API outcome, and resource budget are recorded in a
manifest; downloaded data never silently becomes a release artifact.

| Data class | Location | PR | Nightly | Release |
| --- | --- | --- | --- | --- |
| Upstream conformance vectors | `third_party/corpus/libwebp-test-data` | 68-file selected list | pinned selected vectors | supported vectors × public API matrix |
| libwebp reference output | `third_party/oracle/libwebp` and generated outputs | selected supported features | pairwise encoder set and differential output | all supported encoder options |
| Real-image benchmark | `third_party/benchdata/clic` | none | CLIC validation | CLIC validation plus approved larger splits |
| Structured hostile input | `tests/fixtures/generated` | all committed fixtures | generated matrix plus fuzz corpus | all fixtures under normal and tight limits |
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

Run `tools/index-upstream-smoke-corpus.sh` after fetching libwebp-test-data.
It creates ignored, Rust-readable SHA-256 sidecars for the selected 68 vectors.
The M1 VP8L lossless vectors are `MustAccept` entries with dimensions and a
recorded-libwebp canonical RGBA SHA-256; remaining vectors stay
`ImplementationDefined` until their public decoder path is implemented.

The reference checkout is test-only. It supplies the upstream fuzz dictionary,
future `cwebp` pairwise encoder vectors, `webpmux` metadata vectors, and
animation oracle outputs. It is not linked into the published Rust codec.

Build `cwebp` from that checkout, then run
`tools/generate-reference-corpus.sh`. It produces the 36-vector quality/method
matrix outside Git and writes Rust-readable sidecars containing the resolved
oracle revision, source-image SHA-256, and exact encoder arguments.

Run `python3 tools/generate-reference-edge-corpus.py` to add the separate
66-vector RGB/RGBA edge matrix: 1×1, odd dimensions, a long row, alpha, lossy,
lossless, and near-lossless settings. Each sidecar contains the oracle-decoded
canonical RGBA SHA-256 for later pixel-golden promotion.

## Generated fixtures

Run `cargo run -p xtask -- fixtures generate-malformed` after changing the
generator. It regenerates the committed minimal RIFF/VP8X hostile samples and
their SHA-256 manifests. A discovered failure is minimized before being moved
to `tests/fixtures/regressions/`, with its issue/source, expected result, and
the API path that previously failed.

Use `tools/promote-regression.sh <input.webp> <id> <issue-or-source> <license>`
to create the fixture and its SHA-256 sidecar together. The first manifest is
`MustReject`, so it immediately runs one-shot, `ReadInfo`, and incremental
public API rejection checks; valid-image regressions must be upgraded with the
appropriate accepted API and golden fields.

Animation and metadata vectors must be generated from libwebp tools, retaining
the resolved oracle commit, raw RGBA input, WebP output, per-frame composed RGBA hashes,
rectangles, duration, blend/dispose flags, loop count, background color,
oracle revision, and generator arguments. Metadata generation covers ICCP,
EXIF, XMP, their combinations, boundary payload lengths, chunk order/padding,
duplicates, incorrect declared sizes, and truncation.

`tools/generate-animation-corpus.sh` creates the initial two-frame loop and a
Rust-readable `ReadInfo` sidecar. The animation test is deliberately separate
from pixel decode until frame composition is exposed by the public API.

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
