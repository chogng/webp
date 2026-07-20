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
The remaining work is therefore entropy decoding rather than inverse color
transforms or final RGBA packing.

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
to cache pressure; predictor-block traversal and subtract-green/RGBA fusion
were neutral or slower. Closing the remaining gap requires reducing the four
independent checked symbol decodes per literal, while preserving work-budget
and truncation semantics, rather than further transform-loop tuning.

## Applying the gates to later milestones

M0 owns the reusable fixture, corpus-pin, and benchmark infrastructure. Each
codec milestone owns its path-specific benchmark and profile. M2 and later
milestones must establish their performance baseline when the first public
decode path lands, then run it before claiming milestone completion.
