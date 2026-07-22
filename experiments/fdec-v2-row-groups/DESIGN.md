# FDEC v2 row-group experiment

## Scope

This research-only feature appends one ignorable `FDEC` RIFF chunk to an
otherwise byte-identical standard VP8L fallback. The only fallback mutation is
the RIFF length. Default encode/decode and the public API do not select or
create this representation.

The candidate owns a version-2, checked header and fixed-width directory. Each
directory entry identifies one independent row group and its compressed RGB
representation plus, for alpha images, an independently compressed alpha
plane. A valid group contains complete rows. Groups are deterministic: choose
the largest positive count of complete rows whose decoded representation is at
most 1 MiB; if one legal row exceeds that limit it is its own group.

## Owned invariants

- The directory gives every payload an offset, compressed length, decoded
  length, codec, transform, group CRC, and row range. All arithmetic is
  checked, ranges are non-overlapping and fully contained, and reserved bytes
  are zero.
- Practical uses independent Zstd level-1 frames and Row-Sub RGB. Fastest uses
  independent safe-Rust LZ4 blocks and raw RGB. Alpha is always a separate,
  independently compressed lossless plane; RGB remains exact for transparent
  pixels.
- Decoding allocates only final RGBA plus one group representation/scratch.
  It decompresses, inverse-transforms, interleaves RGB and alpha, and hashes
  each output byte once before moving to the next group. It has no full-frame
  residual allocation.
- A row-range helper selects and decodes only overlapping groups. It is private
  and exists to prove framing independence; it does not change public decode.
- Bad/unknown/duplicate/over-limit/corrupt v2 uses the normal VP8L fallback if
  the outer RIFF is valid. Malformed outer RIFF retains normal decoder errors.
- The v2 CRCs bind the private representation to itself and detect corruption;
  they are not authenticity or semantic-equivalence proof for untrusted input.
  Opportunistic selection is feature/policy gated and disabled by default.

## Non-goals

No third codec, classifier, dictionary training, parameter search, SIMD,
threads, unsafe code, public FDEC enum, or claim of standard WebP support.
This branch is not a product migration even if gates pass.

## Gates

Mechanism and exactness precede timing. The candidate must demonstrate boundary
matrices, per-group independent decode, one-group high-water scratch, no
full-frame residual ownership, one RGBA/CRC visit, private selection for every
alpha input, exact standard fallback bytes, and feature-disabled identity.
Only then may the locked five-round O/B timing and 229-image corpus gates run.
