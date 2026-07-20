# WebP test corpus contract

The test corpus is deliberately split into five sources. A fixture's source,
license, SHA-256, expected API outcome, and resource budget are recorded in a
manifest; downloaded data never silently becomes a release artifact.

| Data class | Location | PR | Nightly | Release |
| --- | --- | --- | --- | --- |
| Upstream conformance vectors | `third_party/corpus/libwebp-test-data` | 64-file selected list once supported | all `.webp` vectors | all vectors × public API matrix |
| libwebp reference output | `third_party/oracle/libwebp` and generated outputs | selected supported features | pairwise encoder set and differential output | all supported encoder options |
| Real-image benchmark | `third_party/benchdata/clic` | none | CLIC validation | CLIC validation plus approved larger splits |
| Structured hostile input | `tests/fixtures/generated` | all committed fixtures | generated matrix plus fuzz corpus | all fixtures under normal and tight limits |
| Historical regressions | `tests/fixtures/regressions` | all | all | all |

## Rolling upstream inputs

`tools/corpus-lock.toml` names the allowed upstream URLs and branches. Fetch
only through:

```text
tools/fetch-libwebp-test-data.sh
tools/fetch-libwebp-oracle.sh
tools/verify-upstream-smoke.sh
```

Each invocation fetches the current `main`, checks out its resolved commit, and
prints that commit. The reference-index script writes the resolved oracle
commit into every generated sidecar. Preserve this output in a release run log
when a replayable snapshot is required.

The reference checkout is test-only. It supplies the upstream fuzz dictionary,
future `cwebp` pairwise encoder vectors, `webpmux` metadata vectors, and
animation oracle outputs. It is not linked into the published Rust codec.

Build `cwebp` from that checkout, then run
`tools/generate-reference-corpus.sh`. It produces the 36-vector quality/method
matrix outside Git and writes Rust-readable sidecars containing the resolved
oracle revision, source-image SHA-256, and exact encoder arguments.

## Generated fixtures

Run `cargo run -p xtask -- fixtures generate-malformed` after changing the
generator. It regenerates the committed minimal RIFF/VP8X hostile samples and
their SHA-256 manifests. A discovered failure is minimized before being moved
to `tests/fixtures/regressions/`, with its issue/source, expected result, and
the API path that previously failed.

Animation and metadata vectors must be generated from libwebp tools, retaining
the resolved oracle commit, raw RGBA input, WebP output, per-frame composed RGBA hashes,
rectangles, duration, blend/dispose flags, loop count, background color,
oracle revision, and generator arguments. Metadata generation covers ICCP,
EXIF, XMP, their combinations, boundary payload lengths, chunk order/padding,
duplicates, incorrect declared sizes, and truncation.

`tools/generate-animation-corpus.sh` creates the initial two-frame loop and a
Rust-readable `ReadInfo` sidecar. The animation test is deliberately separate
from pixel decode until frame composition is exposed by the public API.

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
