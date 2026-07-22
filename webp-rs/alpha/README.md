| Encoder / iteration | Revision | Exact alpha | Whole-image median (3 x 10) ↓ | Throughput ↑ | Cost ↓ | Change from prior Rust | Time vs paired libwebp | Rust ALPH-only median ↓ | ALPH throughput ↑ | ALPH cost ↓ | ALPH change from prior |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| pinned libwebp | `733c91e` | 41/41 | **9934.306 ms** | 6.426 MPix/s | 155.622 ns/pixel | reference | reference | n/a: no public standalone ALPH encoder | n/a | n/a | n/a |
| Rust I1: current `main` baseline | `5e54dd3` | 41/41 | 8058.452 ms | 7.922 MPix/s | 126.237 ns/pixel | baseline | -18.79% | 1786.003 ms | 35.742 MPix/s | 27.978 ns/pixel | baseline |
| Rust I2: batched LSB writer | `7cd8fd4` | 41/41 | 7122.629 ms | 8.962 MPix/s | 111.577 ns/pixel | **-11.61%** | -28.14% | 879.106 ms | 72.615 MPix/s | 13.771 ns/pixel | **-50.78%** |
| Rust I2f: ownership/filter/parser cleanup | pre-I3 checkpoint | 41/41 | 7022.180 ms | 9.091 MPix/s | 110.003 ns/pixel | -1.41% | -29.27% | 800.482 ms | 79.747 MPix/s | 12.540 ns/pixel | -8.94% |
| Rust I3: plane codes + indexed alpha | `1b6bfdb` | **41/41** | **7037.887 ms** | **9.070 MPix/s** | **110.249 ns/pixel** | **+0.22% regression** | **-29.16%** | **794.383 ms** | **80.359 MPix/s** | **12.444 ns/pixel** | -0.76% |

| Encoder / iteration | ALPH bytes / suite ↓ | ALPH bpp ↓ | ALPH gap to libwebp | ALPH change from prior | Complete WebP bytes / suite ↓ | WebP gap to libwebp | WebP change from prior |
|---|---:|---:|---:|---:|---:|---:|---:|
| pinned libwebp | **4,098,325** | **5.1361** | reference | reference | **6,509,902** | reference | reference |
| Rust I1 | 4,135,772 | 5.1830 | +0.91% | baseline | 6,636,088 | +1.94% | baseline |
| Rust I2 | 4,135,772 | 5.1830 | +0.91% | 0.00% | 6,636,088 | +1.94% | 0.00% |
| Rust I2f | 4,135,741 | 5.1830 | +0.91% | -0.00% | 6,636,056 | +1.94% | -0.00% |
| Rust I3 | **4,118,622** | **5.1615** | **+0.50%** | -0.41% all files / **-10.98% structured** | **6,618,910** | **+1.67%** | -0.26% |

# ALPH encoder benchmark and optimization record

The opening tables are the decision ledger. Lower elapsed time, ns/pixel, and
byte counts are better; higher MPix/s is better. A standalone optimization is
called material only when it improves a primary metric by at least 10%.
Sub-10% compatible changes may be folded into an architectural iteration, but
are recorded as marginal rather than presented as wins. Regressions remain in
the table.

At the current operating point Rust uses **29.16% less whole-image time** than
libwebp, which is **41.15% higher throughput**. It is close to, but does not yet
claim, the 50% throughput target. Complete output is 1.67% larger and ALPH is
0.50% larger. There is no honest cross-library ALPH-only speed ratio because
libwebp does not expose a public standalone ALPH encoder; its public whole-image
API is the comparison boundary.

## Benchmark contract

- Profile: lossy VP8 RGB at quality 75 plus lossless ALPH, fast alpha-filter
  selection, alpha quality 100. Alpha is exact; RGB bitstreams are
  encoder-specific and are not claimed to be identical.
- Corpus: 41 transparent upstream files pinned through
  `tools/corpus-lock.toml` at `libwebp-test-data` revision `06ddd96e`. The matrix
  spans 16x16 through 2048x2048, 1 through 256 alpha levels, all four source
  filter labels, color-cache and transform fixtures, natural structured alpha,
  and a 2048x2048 random-alpha stress image.
- Work per run: 41 files x 10 encodes = 410 encodes and 63,836,040 timed pixels.
  One untimed inspection encode per file and compilation are excluded. Each
  table duration is the median of three fresh process runs.
- Baseline: libwebp commit
  `733c91e461c18cf1127c9ed0a80dccbcfed599d3`, built as the repository's pinned
  scalar-canonical oracle. Both public APIs receive the same decoded RGBA.
- Host: Apple arm64, Darwin 25.4.0, Rust 1.97.1, Apple clang 21.0.0. Results
  from another host belong in a separate table.
- Size accounting: `ALPH bytes` includes the one-byte ALPH header. `WebP bytes`
  is the complete RIFF file. Sizes are deterministic and shown for one suite;
  timed logs report ten-suite totals.
- Runner output: machine-readable `metadata`, `case`, `measurement`, and
  `aggregate` records. Per case it reports shape, alpha cardinality,
  transparent/translucent counts, selected ALPH method/filter, output bytes,
  ALPH bytes, bpp, and raw ratio. Per measurement it reports elapsed time,
  MPix/s, and ns/pixel.

Run one measurement from the repository root:

```sh
./tools/benchmark-alpha-encode.sh 10
```

Run the formal three-process series:

```sh
for run_id in 1 2 3; do
  ./tools/benchmark-alpha-encode.sh 10 > "/tmp/alpha-v3-${run_id}.log"
done
```

An isolated worktree may reuse the pinned corpus and oracle:

```sh
WEBP_ALPHA_BENCH_CORPUS=/path/to/libwebp-test-data \
WEBP_ALPHA_BENCH_LIBWEBP=/path/to/libwebp \
./tools/benchmark-alpha-encode.sh 10
```

## Corpus-level size detail

The random stress image is 65.7% of suite pixels and more than 96.5% of each
encoder's ALPH bytes. It intentionally checks incompressible behavior, but it
hides transform gains on useful structured alpha. Therefore both the all-41 total
and the 40-file structured subtotal are mandatory; the latter is not a
replacement or a cherry-picked headline.

| Corpus group | Files | Pixels | Alpha levels | I1 ALPH | I3 ALPH | I3 vs I1 | libwebp ALPH | I3 gap |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| 128x128, 16 levels | 8 | 131,072 | 16 | 57,032 | 52,224 | -8.43% | 52,080 | +0.28% |
| 16x16 binary fixtures | 20 | 5,120 | 2 | 1,060 | 640 | **-39.62%** | 500 | +28.00% |
| 1-15-level structured | 6 | 999,536 | 1-15 | 33,862 | 26,898 | **-20.57%** | 19,064 | +41.09% |
| higher-cardinality structured | 6 | 1,053,572 | 64-256 | 63,946 | 59,000 | -7.73% | 48,877 | +20.71% |
| 2048x2048 random stress | 1 | 4,194,304 | 256 | 3,979,872 | 3,979,860 | -0.00% | 3,977,804 | +0.05% |
| **40 structured files** | **40** | **2,189,300** | **1-256** | **155,900** | **138,762** | **-10.99%** | **120,521** | **+15.14%** |
| **all 41 files** | **41** | **6,383,604** | **1-256** | **4,135,772** | **4,118,622** | **-0.41%** | **4,098,325** | **+0.50%** |

Representative files make the direction and remaining gaps visible. Repeated
fixtures are fully included in the group totals above rather than duplicated
in this table.

| Representative input | Shape | Levels | I1 ALPH | I3 ALPH | Delta | libwebp | I3 gap |
|---|---:|---:|---:|---:|---:|---:|---:|
| `alpha_filter_0_method_0.webp` | 128x128 | 16 | 7,129 | 6,528 | -8.43% | 6,510 | +0.28% |
| `alpha_filter_1.webp` | 16x16 | 2 | 53 | 32 | **-39.62%** | 25 | +28.00% |
| `dual_transform.webp` | 100x30 | 2 | 381 | 189 | **-50.39%** | 184 | +2.72% |
| `lossless4.webp` | 256x256 | 15 | 3,801 | 3,161 | **-16.84%** | 2,648 | +19.37% |
| `lossy_alpha1.webp` | 1000x307 | 15 | 10,854 | 9,077 | **-16.37%** | 6,625 | +37.01% |
| `lossy_alpha2.webp` | 1000x307 | 10 | 10,388 | 8,545 | **-17.74%** | 6,016 | +42.04% |
| `lossy_alpha3.webp` | 1000x307 | 3 | 8,419 | 5,908 | **-29.83%** | 3,575 | +65.26% |
| `alpha_color_cache.webp` | 588x97 | 91 | 1,964 | 1,820 | -7.33% | 1,641 | +10.91% |
| `big_endian_bug_393.webp` | 256x256 | 256 | 16,801 | 16,187 | -3.65% | 16,185 | +0.01% |
| `lossless1.webp` | 1000x307 | 256 | 14,106 | 12,770 | -9.47% | 9,537 | +33.90% |
| `lossy_alpha4.webp` | 100x100 | 64 | 2,863 | 2,683 | -6.29% | 2,440 | +9.96% |
| `lossless_big_random_alpha.webp` | 2048x2048 | 256 | 3,979,872 | 3,979,860 | -0.00% | 3,977,804 | +0.05% |
| `one_color_no_palette.webp` | 100x100 | 1 | 19 | 18 | -5.26% | 16 | +12.50% |

## Iteration log

### I0 - complete literal ALPH encoder (`72c1309`)

Established validation, quality preprocessing, four filters, raw fallback,
headerless VP8L emission, RIFF integration, and pinned-`dwebp` exact decoding.
The historical nine-file v1 run emitted 3,348,150 ALPH bytes over 50 suites and
took 618.958 ms. Those numbers remain historical and are not mixed with v3.

### I1 - greedy LZ77 and adaptive Huffman (`22fb0ec`)

Added bounded greedy backward references, measured Huffman frequencies,
code-length RLE, and a bounded token cache. On the same historical v1 runner,
ALPH size fell 14.75%, while time regressed 12.95%. The size win was material
and the time trade remained explicit. This is the code baseline at `5e54dd3`
for the broader v3 table.

### Benchmark v3 - broader evidence (`8de284e`)

Expanded the public comparison and exact external oracle from nine highly
duplicated inputs to all 41 transparent upstream files. Added machine metadata,
per-file content/size metrics, ns/pixel, and an isolated Rust ALPH profile. This
changes measurement coverage, not encoder output.

### I2 - batched LSB-first writes (`7cd8fd4`)

Replaced one-bit-at-a-time emission with bounded byte-window merges in the
shared core `BitWriter`. Output stayed byte-for-byte identical. Whole-image
time fell **11.61%** and ALPH-only time fell **50.78%**, so this is the material
speed iteration. The disproportionate ALPH result identifies bit emission as
the former alpha hot path.

### I2f - folded ownership, filtering, and parser cleanup

Borrowed quality-100 input instead of copying it, filtered by rows instead of
using per-pixel division/modulo, moved token ownership to a private bounded
LZ77 module, and sized its match table to the input. Relative to I2, whole time
fell 1.41% and ALPH-only time fell 8.94%. Neither clears the 10% rule, so these
are folded support changes rather than standalone wins.

### I3 - VP8L plane distance codes and color indexing (`1b6bfdb`)

Added nearby two-dimensional distance codes and a row-packed VP8L
color-indexing transform for planes with at most 16 levels. Small inputs encode
both indexed and plain forms and retain the smaller result; larger low-cardinality
planes take the indexed path directly. The palette subimage and indexed entropy
stream use the existing adaptive Huffman machinery.

Against I2f, whole time regressed 0.22% and ALPH-only time improved 0.76%, both
noise-level and below the threshold. Size is the accepted result: the 40-file
structured subtotal fell **10.98%**, with representative low-cardinality files
improving 16.37% to 50.39%. The all-41 ALPH total fell only 0.41% because the
incompressible random plane dominates it. All 41 outputs decoded to the exact
source alpha through pinned `dwebp`.

From the latest-main I1 baseline through I3, whole time is down **12.66%**,
ALPH-only time is down **55.52%**, complete size is down 0.26%, and ALPH size is
down 0.41% across all files.

## Rejected and non-material experiments

Diagnostic probes below used the same code base and corpus stated in each row,
but not all were three-process formal runs. They are decision evidence, not
primary headline measurements.

| Probe | Evidence | Result | Decision |
|---|---|---|---|
| plane distance codes alone | 41-file structured subtotal | -5.51% ALPH | useful only when grouped with a larger transform architecture |
| plane distance codes alone | historical 128x128 fixture | 7,129 to 7,150 bytes, +0.29% | explicit local regression; do not present alone |
| four hash candidates | historical nine-file probe | 7,129 to about 7,499 bytes on the 128x128 case; ALPH time about +30.5% | rejected |
| four candidates + plane codes | historical nine-file probe | 7,129 to about 7,527 bytes; ALPH time about +44.4% | rejected |
| one-step lazy parsing | candidate-parser probe | about -0.04% from the already worse candidate result | rejected as immaterial |
| alternate Huffman heap | nine-file timing probe | no size change and about +2.2% time | rejected |
| I2f cleanup as independent win | formal v3 | -1.41% whole / -8.94% ALPH-only | retained only as folded architecture support |

## Research basis and next architecture targets

- The [WebP lossless bitstream specification](https://developers.google.com/speed/webp/docs/webp_lossless_bitstream_specification)
  defines LZ77, prefix coding, color indexing, the color cache, and optional
  spatial entropy groups.
- [RFC 9649](https://www.rfc-editor.org/rfc/rfc9649.html) specifies nearby
  two-dimensional distance codes 1 through 120 and the linear fallback.
- Google's [WebP lossless and alpha study](https://developers.google.com/speed/webp/docs/webp_lossless_alpha_study)
  reports that two-dimensional locality and color caching improve density on a
  much larger translucent-image population. The 41-file conformance corpus is
  still a gate, not a substitute for a real-image dataset.
- Larmore and Hirschberg's
  [Package-Merge paper](https://ics.uci.edu/~dhirschb/pubs/LenLimHuff.pdf)
  gives an optimal length-limited prefix-code construction. It should be tried
  only after diagnostics show Huffman lengths are a material owner.
- The pinned libwebp implementation uses quality-scaled hash chains, explicit
  previous-pixel/previous-row candidates, several reference strategies, lazy
  reach decisions, plane codes, and raw fallback. These are algorithmic
  references, not module boundaries to copy.

The next accepted architecture should target at least one measurable 10% gap:

1. **Structured ALPH density:** Rust is still 15.14% above libwebp on the
   40-file structured subtotal. A costed choice among palette, color cache,
   row/RLE, and bounded multi-candidate parses is the leading target. The
   rejected unconditional candidate walk shows that more search alone is not
   enough; token cost must govern it.
2. **Real-image evidence:** add a pinned, licensed translucent PNG/WebP corpus
   with PSNR/SSIM or exact-alpha gates, alpha-cardinality buckets, p50/p95
   latency, and peak RSS. No architecture should be tuned only to conformance
   fixtures.
3. **Whole-image 50% throughput target:** current throughput is 41.15% above
   libwebp. Reaching exactly 50% requires only another 5.9% Rust time reduction,
   which is below the project's standalone significance rule. It should be
   bundled with a >=10% density, p95, memory, or broader-dataset improvement.

## Resource behavior

- Match heads scale to twice the next power of two of input samples, clamped to
  256 through 65,536 `u32` entries (1 KiB through 256 KiB).
- Inputs through 4,194,304 samples may cache one packed `u32` token per sample;
  larger inputs use a second bounded parse instead of retaining unbounded token
  state.
- Quality-100 input is borrowed. Lower qualities own their quantized plane.
- Indexed planes retain a packed row buffer at 1/2, 1/4, or 1/8 of the source
  size depending on palette cardinality, plus at most 16 palette entries.
- Peak RSS is not yet emitted by v3 and remains a required metric for the next
  real-image benchmark revision.

## Correctness and acceptance gates

Every accepted iteration must pass:

- exact Rust round-trip for every ALPH compression/filter combination;
- exact pinned-`dwebp` decode for all 41 alpha-quality-100 files;
- Rust/`dwebp` agreement for the quality 0, 70, and 99 reduction matrix;
- workspace tests, clippy with warnings denied, formatting, and Bazel tests;
- three ten-iteration v3 runs on the same pinned corpus, oracle, host, and
  release profile;
- explicit size, speed, and regression reporting, including rejected results
  when no primary metric improves by at least 10%.

Before a benchmark worktree is created, record `git rev-parse main`, refresh
the local `main` reference when needed, create the worktree from that exact
revision, and verify that the recorded `main` commit is its ancestor. This
series used latest `main` `5e54dd37c14cc0c810d5a2283b644161ddb2a9b2` as
its base; stale worktree measurements are not eligible for this table.
