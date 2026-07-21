# Quality gates for codec milestones

Correct decoding is necessary but not sufficient for a milestone to be
complete. Every codec milestone must meet the following gates before being
labelled complete.

## Required gates

1. **Conformance and compatibility.** Public APIs must pass their pinned
   corpus fixtures and canonical output checks.
2. **Robustness.** Bounds, resource limits, malformed inputs, and relevant
   fuzz targets must pass. New parsing or entropy paths require a direct fuzz
   target or a documented reason why an existing target covers them.
3. **Performance.** The changed public path must have a repeatable release
   benchmark using representative local data. It must record the command,
   corpus identity, work performed, elapsed time, and throughput. A change may
   not regress its established median by more than 5% without an explicit,
   reviewed justification.
4. **Resource behaviour.** The benchmark/profile report must identify output
   allocation, peak retained working data, and the top CPU or allocation
   hotspots. New limits must be exercised by tests.

When a pinned oracle exists, the performance report must also include an
in-process comparison using equivalent decode work. Matching the oracle's
throughput is not an automatic requirement, but any material gap must have an
owner and a remediation plan. A milestone with an unprofiled material gap is
**performance pending**, not complete.

## VP8L M1 baseline

Run the VP8L comparison after fetching the pinned corpus and oracle:

```sh
bash tools/benchmark-vp8l.sh 5
```

The script selects every `MustAccept` VP8L decode fixture, preloads its input,
and reports the public Rust decoder and libwebp's `WebPDecodeRGBA` C API over
the same work. The native helper and Rust runner both include RGBA allocation
and pixel reconstruction but exclude input file I/O and process launch.

The local 2026-07-20 baseline covers 41 files (14.4 MB compressed) and 205
decodes / 114.8 MB RGBA per run. Across three five-iteration runs, the median
was 1.894 s (60.6 MB/s) for Rust and 0.518 s (221.4 MB/s) for libwebp: a 3.65x
gap. M1 is therefore functionally complete but performance pending.

## VP8L encoder baseline procedure

The public static encoder has a matching release benchmark. It decodes each
pinned `MustAccept` VP8L fixture once before timing, then measures only
`encode_lossless_rgba` over the retained straight-RGBA inputs:

```sh
bash tools/benchmark-vp8l-encode.sh 5
```

The 2026-07-20 single-pass Rust baseline processed 41 inputs, 22,954,432 RGBA
bytes, and produced 18,301,768 WebP bytes in 623.300 ms (checksum `18305130`).

The benchmark now also builds a native helper against the exact libwebp commit
in `corpus-lock.toml` and calls `WebPEncodeLosslessRGBA` over the same retained
41 RGBA inputs. Three five-iteration runs measured Rust at 2.985 s, 2.978 s,
and 2.983 s, with a 2.983 s median (38.5 MB/s). Pinned libwebp measured 8.828
s, 8.949 s, and 8.837 s, with an 8.837 s median (13.0 MB/s); Rust is 2.96x
faster. Rust produces 18,301,768 bytes per matrix versus libwebp's 14,176,624,
a 1.291x size ratio. This is a product comparison between Rust's bounded
transform/cache/LZ77 profile and libwebp's default lossless effort, rather than
a claim of equal encoder search work.

## VP8 static encoder baseline

The bounded static intra16 VP8 profile has a release matrix benchmark over the
21 locked `reference-v1` lossy inputs. It decodes those inputs once before
timing, then encodes every retained RGBA image at quality 0, 75, and 100:

```sh
bash tools/benchmark-vp8-encode.sh 1
```

The 2026-07-20 baseline performed 63 encodes over 4,128,768 RGBA bytes in
142.349 ms, produced 288,010 WebP bytes, and reported checksum `293176`.
Encoder working data is bounded by macroblock-padded source YUV planes, equally
sized reconstructed YUV planes, and one coefficient record per macroblock;
the 16-mode candidate loop retains only its winning reconstruction. The main
CPU cost is therefore the bounded transform/quantize/reconstruct scoring loop,
not container serialization. Locked `dwebp` pixel-oracle tests cover quality
0/75/100, alpha, and a multi-macroblock image. A pinned-libwebp encode-rate
comparison and a reviewed regression threshold remain M9 work.

## VP8 factored intra16 search

The first M9 encoder pass uses the independence of VP8's intra16 luma and
chroma predictions. The original bounded search evaluated all 16 mode pairs,
repeating identical luma transform/reconstruction work for each chroma mode
and vice versa. The encoder now evaluates four luma candidates and four
chroma candidates separately, combines their independently optimal
lexicographic distortion/coefficient scores, and reconstructs the selected
pair once. A direct differential test proves the factored result equals the
previous exhaustive 16-pair search.

Three five-iteration quality-matrix runs measured 313.775 ms, 312.878 ms, and
312.921 ms. The 312.921 ms median is 62.584 ms per complete 63-encode matrix,
56.0% faster than the 142.349 ms M7 baseline. Output remains 288,010 bytes and
checksum `293176` per matrix, so the optimization is bit-for-bit stable on all
21 locked inputs at quality 0, 75, and 100.

## VP8 plane-specific candidate reconstruction

After factoring the search, candidate scoring still sent every luma-only or
chroma-only candidate through full macroblock reconstruction. The encoder now
dequantizes, inverse-transforms, predicts, and combines only the plane family
being scored; only the selected luma/chroma pair receives a full reconstruction.
RGBA-to-YUV preparation also fills each 2x2 luma group while accumulating its
single chroma sample, avoiding a second read of every padded RGB pixel. Unit
tests compare both plane-specific reconstruction paths with full reconstruction
for all 16 mode pairs.

Three five-iteration runs measured 276.972 ms, 270.770 ms, and 273.375 ms.
The 273.375 ms median is 12.6% faster than the preceding 312.921 ms result and
54.675 ms per complete quality matrix, 61.6% faster than the original M7
baseline. Output remains 288,010 bytes with checksum `293176` per matrix.

## VP8 pinned-libwebp encoder comparison

`benchmark-vp8-encode.sh` now builds a native helper against the exact
libwebp commit in `corpus-lock.toml`. Both encoders retain the same 21 decoded
RGBA inputs, encode quality 0/75/100 in the same order, include output
allocation, and exclude input I/O and process launch from timing. The Rust
profile is the documented bounded intra16 encoder; libwebp uses its public
`WebPEncodeRGBA` default profile, so size and speed are product comparisons
rather than claims of identical encoder effort.

Across three five-iteration runs, Rust measured 265.411 ms, 263.926 ms, and
263.045 ms; libwebp measured 328.294 ms, 335.329 ms, and 331.320 ms. The
median Rust run is 20.3% faster. Rust produced 288,010 bytes per matrix versus
135,226 bytes for libwebp, however, making the bounded profile 2.13x larger.
VP8 encoder speed therefore has no material remediation gap on this matrix;
rate/quality tooling and coefficient/probability decisions own the remaining
encoder gap.

## VP8 zero-residual macroblock skipping

The encoder now marks macroblocks whose quantized Y2, luma, U, and V
coefficients are all zero as skip candidates. It derives VP8's skip
probability from the observed macroblock ratio, emits a skip-aware first and
token partition pair, and retains it only when its exact combined byte length
is smaller than the regular pair. Skipped blocks reset every neighbouring
coefficient context exactly as the decoder does. A 64x64 all-zero-residual
frame exercises probability zero, omits all coefficient tokens, and decodes
identically in Rust and the pinned `dwebp` oracle.

The locked 21-file quality matrix did not select the skip-aware pair, so this
pass intentionally records no corpus rate win: all three five-iteration runs
still produced 288,010 bytes and checksum `293176` per matrix. Rust measured
278.281 ms, 272.782 ms, and 274.468 ms, with a 274.468 ms median; the matching
libwebp median was 334.754 ms. The next rate pass must therefore target the
non-zero coefficient stream rather than flat macroblocks.

## VP8 frame-adaptive coefficient probabilities

The encoder now records both outcomes for every VP8 coefficient-tree node,
indexed by coefficient type, band, neighbour context, and node. It derives a
candidate probability table from the complete frame and transmits only updates
whose estimated token savings cover their update flag and eight-bit literal.
The estimate uses a deterministic fixed-point log approximation only as a
shortlist: default and adapted first/token partition pairs are both encoded,
and the adapted pair is retained only when its exact combined byte length is
smaller. The same selection is applied independently to regular and
macroblock-skip profiles. A repeated non-zero distribution test requires real
probability updates, while the pinned `dwebp` matrix validates their decoded
pixels.

Three five-iteration runs measured 374.015 ms, 375.813 ms, and 373.553 ms for
Rust, with a 374.015 ms median. The matching libwebp runs measured 329.267 ms,
333.221 ms, and 330.766 ms, with a 330.766 ms median, so Rust is now 13.1%
slower after the additional statistics and candidate encoding work. Output
fell from 288,010 to 183,802 bytes per matrix, a 36.18% reduction, with
benchmark checksum `188968`; locked-oracle decoded pixels remain unchanged.
The remaining size ratio is 1.359x against libwebp's 135,226 bytes, down from
2.13x. Subsequent VP8 work should add an explicit rate/distortion gate before
changing mode selection and should recover the duplicate candidate-encoding
CPU cost.

## VP8 rate/distortion comparison

The pinned VP8 encoder comparison now performs one untimed encode/decode pass
per input and quality after the timed loop. It reports output bytes, aggregate
RGB sum-squared error, and RGB PSNR independently for quality 0, 75, and 100;
alpha remains covered by the exact oracle tests and is excluded from the
distortion score. Each product decodes its own output, while the locked corpus
and cross-decoder tests ensure both begin from equivalent retained RGBA inputs.
Quality measurement is outside `elapsed_ms`, so the established encode-speed
series remains comparable.

At the frame-adaptive probability baseline, Rust produces 6,942, 42,022, and
134,838 bytes with PSNR 25.857, 37.376, and 48.650 dB. Pinned libwebp produces
5,968, 32,344, and 96,914 bytes with PSNR 27.148, 38.232, and 49.421 dB.
Rust therefore trails by 1.291, 0.856, and 0.771 dB while also using more
bytes. A trial that changed intra16 selection from absolute error to squared
error gained only 0.004--0.018 dB and added 412 bytes per matrix, so it was
rejected. Future mode or quantization changes must improve this joint rate and
distortion record rather than optimizing size alone.

## VP8 fused coefficient observation

The adaptive-probability pass originally traversed every coefficient tree once
to write the default token partition and again to collect node statistics.
The entropy writer now offers a crate-private observed path that reports each
adaptive node outcome while emitting the same boolean-coded bit. The default
partition therefore produces its probability statistics in one traversal;
the public coefficient encoder remains unchanged. The locked matrix retains
exactly 183,802 bytes, checksum `188968`, per-quality byte counts, RGB SSE, and
PSNR from the rate/distortion baseline.

Three five-iteration runs measured 355.789 ms, 352.321 ms, and 353.658 ms for
Rust, with a 353.658 ms median, 5.4% faster than the preceding 374.015 ms.
Pinned libwebp measured 335.518 ms, 331.127 ms, and 335.493 ms, with a
335.493 ms median, leaving Rust 5.4% slower on the comparison matrix. The
remaining duplicate work is the intentionally retained default/adapted boolean
encoding required by the exact no-expansion decision.

## VP8L entropy-path optimization record

The 2026-07-20 optimization pass retained the same 41-file corpus, five
iterations, public API, and checksum (`96355`). It replaced per-bit extraction
with a bounded five-byte LSB-first window, made the cross-crate hot paths
inlineable, and avoids per-pixel or per-copy capacity checks after the decoder
has already reserved the validated image size. It also caches the input bit
length in the reader. Allocation failures remain reported for generic output
sinks that have exhausted their capacity.

Three post-change runs measured Rust at 1.405 s, 1.407 s, and 1.440 s; its
median is 1.407 s (81.6 MB/s), a 25.7% improvement over the established Rust
baseline. The corresponding libwebp median was 0.531 s (216.0 MB/s), leaving
a 2.65x gap. Sampling before this pass identified Huffman symbol decoding,
entropy-image dispatch, LZ77 output expansion, and literal emission as the
dominant entropy-path work; this pass addresses the shared bit-reader and
allocation overhead in those paths. The gap remains performance pending and
the next profile pass should separately quantify inverse transforms and final
RGBA packing.

## VP8L Huffman root-consume refinement

The next 2026-07-20 pass profiles the same corpus by file and finds that
`lossless_big_random_alpha.webp` contributes most of the decode time. Its
high-entropy literal stream repeatedly hits eight-bit root-table entries. The
decoder now consumes that root prefix in one `read_bits(8)` operation. A
shorter root-table code rewinds only its unused tail bits; a longer code keeps
the consumed prefix for the existing fallback. This removes the separate
`peek_bits` and checked `skip_bits` operation from the common root-table hit.

Across three five-iteration runs, Rust measured 1.271 s, 1.280 s, and 1.273
s; the 1.273 s median is 90.2 MB/s, 9.5% faster than the preceding 1.407 s
median and 32.8% faster than the original 1.894 s baseline. The matching
libwebp median was 0.529 s (217.1 MB/s), leaving a 2.41x gap. The dominating
single image improved from about 5.145 s to 4.666 s over 20 decodes of 16 MB
RGBA output. All runs retain checksum `96355` for the complete corpus.

## VP8L cached entropy-reader refinement

File-level measurements show `lossless_big_random_alpha.webp` accounts for
roughly 92% of the complete benchmark time. It is a 2048x2048, 13 MB lossless
stream with subtract-green and spatial meta-Huffman coding; its dominant code
group has full green and alpha alphabets and nontrivial red and blue trees.
This localized the cost to the main VP8L decode path rather than container
parsing, but did not yet distinguish entropy decoding from predictor work.
The phase measurement below makes that distinction.

The next pass keeps a safe 64-bit LSB-first input window and reloads it only
after approximately 32 consumed bits, caches the active meta-Huffman group to
each tile or row boundary, batches the three non-green literal work charges,
and compacts root and secondary lookup entries to four bytes. The compact
secondary table handles 9-to-15-bit codes without the cache expansion seen in
the rejected 16-byte-entry prototype. Its maximum allocation is included in
the meta-group storage bound. The implementation continues to forbid unsafe
code and retains checked tail decoding near end of input.

Three complete five-iteration runs measured Rust at 1.179 s, 1.190 s, and
1.181 s. The 1.181 s median is 97.2 MB/s, 7.2% faster than the preceding
committed 1.273 s result and 37.7% faster than the original 1.894 s baseline.
The corresponding libwebp median was 0.529 s (217.0 MB/s), leaving a 2.23x
gap. The dominating image measured 4.331 s, 4.362 s, and 4.410 s over 20
decodes, with a 4.362 s median. Every complete run retained checksum `96355`.

Wider ten-bit roots and a full-size secondary-table prototype regressed due
to cache pressure; subtract-green/RGBA fusion was neutral or slower. This left
both checked entropy decoding and the still-scalar predictor adapter as
candidates requiring direct phase measurement.

## VP8L block-oriented predictor refinement

Phase timing on `lossless_big_random_alpha.webp` identified two material gaps,
not a long list of small loops. Before this refinement, one decode spent about
97.3 ms in entropy expansion and 109.5 ms in the predictor transform. Color
inversion used about 7.7 ms, subtract-green 0.7 ms, RGBA packing 2.8 ms, and
header/transform parsing 0.1 ms. The predictor and entropy paths were therefore
the only large optimization targets.

The predictor adapter now follows the reference decoder's block-oriented
shape: it handles the fixed first-row and first-column rules separately,
walks horizontal predictor tiles, dispatches the predictor once per tile, and
reconstructs directly in packed ARGB. This removes per-pixel block division,
mode validation, enum dispatch, and repeated conversion of the residual and
four neighbors to channel structs. The implementation remains portable safe
Rust with no architecture-specific intrinsics or unsafe code. A differential
unit test checks the packed implementation against the scalar reference for
all fourteen predictor modes, including the right-edge top-right rule.

Three complete five-iteration runs measured Rust at 0.879 s, 0.903 s, and
0.892 s. The 0.892 s median is 128.7 MB/s, 24.5% faster than the preceding
1.181 s result and 52.9% faster than the original 1.894 s baseline. The
corresponding libwebp runs were 0.516 s, 0.533 s, and 0.525 s, with a 0.525 s
median (218.4 MB/s) and a remaining 1.70x gap. Every run retained checksum
`96355` over all 41 accepted VP8L files.

After the rewrite, the same image's predictor phase fell to roughly 51.5 ms,
a 53% reduction. The next structural target is the entropy core: libwebp keeps
one refillable bit window and compact Huffman-table pointers in the decode
loop, advances them without a `Result` boundary per symbol, and checks stream
state at grouped decode boundaries. Parser and final RGBA packing costs are
too small to warrant dedicated optimization work.

## VP8L packed entropy backend

The next pass isolates the dense pixel stream from the strict parser path.
`BitReader` retains its existing transactional API for headers and other
callers, while an explicitly borrowed shift-register adapter amortizes input
loads inside entropy expansion and synchronizes the checked cursor on drop.
Likewise, the generic validated `HuffmanTable` remains available unchanged;
the pixel decoder converts it to a packed ten-bit-root backend whose direct
and secondary entries are two bytes. Pathological alphabets that do not fit
the packed entry format retain the generic fallback.

The dominant image's entropy phase fell from roughly 97.3 ms to 83.5 ms, a
14% phase improvement. Three complete five-iteration runs measured 0.827 s,
0.834 s, and 0.877 s for Rust, with a 0.834 s median. The matching libwebp
median was 0.525 s. Relative to the preceding committed 0.892 s result this is
a 6.5% end-to-end improvement, but entropy remained the largest individual
gap.

The retained design is intentionally smaller than several rejected
prototypes. A two-symbol table occupying roughly one megabyte regressed from
cache pressure, a specialized single-group dispatch was neutral, and deriving
the absolute cursor instead of maintaining it regressed the dominant image by
about 4%. Those variants were removed. Direct tests cover arbitrary starting
bit offsets, refill boundaries, exact tail EOF behaviour, fifteen-bit codes,
and the generic fallback.

The implementation was developed against the VP8L format description, the
pinned libwebp oracle, and an architectural/performance study of the safe-Rust
[`image-webp`](https://github.com/image-rs/image-webp) decoder. `image-webp` is
not a dependency and no external decoder code is linked into this project.

## VP8L decoupled RGBA predictor backend

Profiling the dominant 2048x2048 image showed that all 4,190,209 non-border
pixels select predictor mode 12, clamped add/subtract. The packed implementation
therefore spent most of its time repeatedly extracting, clamping, and repacking
four channels. Rather than add a file-specific branch, the transform pipeline
now has a private layout boundary: entropy, color, and indexing may retain
packed ARGB, while predictor reconstruction requests an RGBA-byte backend.
The shared `webp-vp8l-transform` crate and all of its callers are unchanged.

The mode-12 kernel receives separate current-row, top-row, and top-left slices
and iterates four-byte channel groups. This exposes non-aliasing and channel
parallelism to the optimizer while staying in safe, portable Rust. Other
predictor modes use the same backend, and a differential test compares all
fourteen modes with the independent specification implementation, including
the right-edge top-right rule. The layout wrapper converts only when transform
order requires it and validates intermediate dimensions against each
transform descriptor rather than coupling them to final image width.

The dominant file fell from roughly 146 ms to 121 ms per decode. Three complete
five-iteration runs measured Rust at 0.700 s, 0.700 s, and 0.697 s; the 0.700 s
median is 164.0 MB/s. The corresponding libwebp runs were 0.534 s, 0.535 s,
and 0.537 s, with a 0.535 s median (214.5 MB/s), leaving a 1.31x gap. Together
with the packed entropy backend, this is 21.5% faster than the preceding
committed 0.892 s result and 63.0% faster than the original 1.894 s baseline.
Every run retained checksum `96355`.

Both layouts are already included in the decoder's conservative allocation
accounting: conversion can briefly retain one packed four-byte pixel buffer
and one four-byte RGBA buffer, matching the previous packed-output plus final
RGBA lifetime. The remaining material optimization target is entropy symbol
expansion; parser and final layout costs remain below the threshold for a
dedicated pass.

## VP8L CLIC real-image decode gate

The 41-file conformance corpus above is strongly dominated by one synthetic
high-entropy image, so it is not sufficient by itself to close M1 performance.
The broader gate uses all 102 RGB images from the pinned CLIC validation
manifest. Pinned `cwebp` generates three exact-lossless streams per image with
methods 0, 3, and 6, for 306 inputs and 755,574,411 decoded pixels per full
pass. Generated files live in the ignored
`third_party/benchdata/clic/vp8l-lossless-exact` cache. Reproduce both corpus
generation and the direct-API comparison with:

```sh
bash tools/benchmark-vp8l-clic.sh 1 4
```

The runner rejects any libwebp checkout other than the commit in
`tools/corpus-lock.toml`, compiles the C benchmark against that checkout's
static library, and passes the identical ordered input set to both decoders.
The 2026-07-21 run used libwebp commit
`733c91e461c18cf1127c9ed0a80dccbcfed599d3`. Both implementations produced
3,022,297,644 RGBA bytes and checksum `997056` per aggregate pass.

Three full-corpus runs measured libwebp at 13.956 s median and Rust at 15.092 s
median. Rust is therefore 1.081x the libwebp time, or 8.1% slower. The Rust
decoder is 27.7% faster than its original 20.863 s median. A method split shows
median pairs of 4.689 s versus 4.478 s for method 0 (Rust 4.5% faster), 4.724 s
versus 5.315 s for method 3 (Rust 12.5% slower), and 4.611 s versus 5.216 s for
method 6 (Rust 13.1% slower), with libwebp listed first in each pair.

The retained optimization keeps the output pixel vector outside the optional
deferred color-cache branch and remaps sparse wire meta-Huffman ids to dense
group indices once during setup. This removes a per-literal enum branch and a
per-meta-run binary search. Predictor residual conversion is also fused with
row reconstruction, keeping each converted row cache-hot instead of writing
and rereading a complete RGBA residual frame. Test-instrumented phase
measurements assign roughly 57% of decode time to entropy expansion, 25--33%
to fused predictor reconstruction, and 3--4% to remaining final layout
conversion, so entropy and predictor remain active optimization owners.

M1 correctness and its original conformance-corpus performance gate are
complete. The reviewed M9 thresholds below explicitly accept this measured
real-image gap while retaining its profiled optimization owners.

## VP8L-frame animation encoder baseline

The public animation encoder has a deterministic synthetic release profile:

```sh
bash tools/benchmark-animation-encode.sh 5
```

It encodes six VP8L rectangles on a 320x240 canvas, covering full and partial
frames, even offsets, transparency, blend/replace, dispose/background, frame
durations, and loop count. Three five-iteration runs measured 90.866 ms,
93.220 ms, and 81.504 ms, with a 90.866 ms median. Each run consumes 2,923,520
RGBA bytes and produces 1,937,440 bytes with checksum `1937850`.

The encoder's retained working data is bounded by the caller-owned 584,704
RGBA bytes, one VP8L frame workspace at a time, the completed compressed frame
payloads, and the final RIFF output. All size arithmetic and allocations use
the existing checked encoder paths. CPU time is dominated by the six VP8L
frame encodes; `VP8X`/`ANIM`/`ANMF` serialization is linear in payload size.

## Reviewed M9 regression thresholds

The following thresholds accept the measured product profiles while retaining
the global rule that a changed path may not regress its committed median by
more than 5% without an explicit review. Cross-libwebp ratios are evaluated
within the same run to reduce host and load sensitivity.

| Public path | Reproduction | Reviewed threshold |
| --- | --- | --- |
| VP8L conformance decode | `bash tools/benchmark-vp8l.sh 5` | Rust median <= 0.735 s, checksum `96355`, and <= 1.40x pinned-libwebp time |
| VP8L CLIC decode | `bash tools/benchmark-vp8l-clic.sh 1 4` | aggregate Rust median <= 15.85 s and <= 1.15x pinned-libwebp time |
| VP8L static encode | `bash tools/benchmark-vp8l-encode.sh 5` | Rust median <= 3.132 s, exact round trips, and output <= 1.35x pinned libwebp |
| VP8 static encode | `bash tools/benchmark-vp8-encode.sh 5` | Rust median <= 371.341 ms, output <= 1.40x pinned libwebp, and PSNR floors 25.807/37.326/48.600 dB at quality 0/75/100 |
| VP8L-frame animation encode | `bash tools/benchmark-animation-encode.sh 5` | Rust median <= 95.409 ms, output <= 406,862 bytes per six-frame animation, and locked `webpmux`/`dwebp` acceptance |

The CLIC decoder's measured 1.081x gap is explicitly accepted for M9 because
it is reproducible, stays inside the 1.15x threshold, and retains profiled
entropy-expansion and predictor-reconstruction owners. It remains a future
optimization target, not an unprofiled milestone blocker. Output-size and PSNR
thresholds are product guards, not bitstream freezes; a reviewed coding-tool
change may update their baselines when conformance and resource gates pass.

## Applying the gates to later milestones

M0 owns the reusable fixture, corpus-pin, and benchmark infrastructure. Each
codec milestone owns its path-specific benchmark and profile. M2 and later
milestones must establish their performance baseline when the first public
decode path lands, then run it before claiming milestone completion.
