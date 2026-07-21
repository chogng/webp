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

## Applying the gates to later milestones

M0 owns the reusable fixture, corpus-pin, and benchmark infrastructure. Each
codec milestone owns its path-specific benchmark and profile. M2 and later
milestones must establish their performance baseline when the first public
decode path lands, then run it before claiming milestone completion.
