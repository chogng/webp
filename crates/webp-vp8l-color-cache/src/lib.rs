#![forbid(unsafe_code)]
//! The VP8L color cache.
//!
//! VP8L hashes complete packed 32-bit colors, including alpha and RGB values
//! of fully transparent pixels.  A cache slot is overwritten every time a
//! decoded pixel is inserted; callers must therefore call [`ColorCache::insert`]
//! for literals, cache hits, and every pixel emitted by a backward reference.

use webp_core::{DecodeError, DecodeErrorKind, WorkBudget};
use webp_vp8l_entropy::copy_lz77_pixels;

/// The smallest color-cache exponent accepted by VP8L.
pub const MIN_COLOR_CACHE_BITS: u8 = 1;
/// The largest color-cache exponent accepted by VP8L.
pub const MAX_COLOR_CACHE_BITS: u8 = 11;
/// VP8L's specified multiplicative hash constant.
pub const COLOR_CACHE_HASH_MULTIPLIER: u32 = 0x1e35_a7bd;

/// A VP8L color cache, addressed by its bitstream cache index.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ColorCache {
    bits: u8,
    entries: Box<[u32]>,
}

/// An ARGB output stream coupled to a VP8L color cache.
///
/// VP8L inserts every emitted pixel into the color cache, including pixels
/// produced by a color-cache reference and every pixel materialized by an
/// overlapping backward reference.  Keeping that sequencing in one small
/// type prevents entropy decoders from accidentally treating cache references
/// as reads only.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ColorCacheOutput {
    pixels: Vec<u32>,
    cache: ColorCache,
}

impl ColorCacheOutput {
    /// Creates an empty output stream backed by `cache`.
    #[must_use]
    pub fn new(cache: ColorCache) -> Self {
        Self {
            pixels: Vec::new(),
            cache,
        }
    }

    /// Creates an empty output stream with a cache of `2^bits` entries.
    pub fn with_cache_bits(bits: u8) -> Result<Self, DecodeError> {
        Ok(Self::new(ColorCache::new(bits)?))
    }

    /// Returns pixels emitted so far in VP8L's packed ARGB representation.
    #[must_use]
    pub fn pixels(&self) -> &[u32] {
        &self.pixels
    }

    /// Returns the cache state after the pixels emitted so far.
    #[must_use]
    pub fn cache(&self) -> &ColorCache {
        &self.cache
    }

    /// Consumes the sink and returns its pixels and final cache state.
    #[must_use]
    pub fn into_parts(self) -> (Vec<u32>, ColorCache) {
        (self.pixels, self.cache)
    }

    /// Emits a literal ARGB pixel and inserts it into the color cache.
    pub fn emit_literal(&mut self, color: u32) -> Result<(), DecodeError> {
        self.emit(color)
    }

    /// Resolves a VP8L color-cache `index`, emits that color, and reinserts it.
    ///
    /// An out-of-range index is an invalid bitstream error and leaves both the
    /// output and cache unchanged.
    pub fn emit_cache_hit(&mut self, index: usize) -> Result<u32, DecodeError> {
        let color = self.cache.get(index)?;
        self.emit(color)?;
        Ok(color)
    }

    /// Emits an overlap-safe VP8L backward reference and caches every output
    /// pixel in emission order.
    ///
    /// Validation, output bounds, allocation failure, and work-budget
    /// exhaustion have the same transactional behavior as
    /// [`copy_lz77_pixels`]: this sink is left unchanged on error.
    pub fn copy_lz77(
        &mut self,
        length: usize,
        distance: usize,
        output_limit: usize,
        budget: &mut WorkBudget,
    ) -> Result<(), DecodeError> {
        let start = self.pixels.len();
        copy_lz77_pixels(&mut self.pixels, length, distance, output_limit, budget)?;
        for &color in &self.pixels[start..] {
            self.cache.insert(color);
        }
        Ok(())
    }

    fn emit(&mut self, color: u32) -> Result<(), DecodeError> {
        self.pixels.try_reserve(1).map_err(|_| {
            DecodeError::new(
                DecodeErrorKind::AllocationFailed,
                None,
                "VP8L output allocation failed",
            )
        })?;
        self.pixels.push(color);
        self.cache.insert(color);
        Ok(())
    }
}

impl ColorCache {
    /// Creates a zero-initialized VP8L color cache with `2^bits` entries.
    ///
    /// VP8L permits exponents from 1 through 11, inclusive.
    pub fn new(bits: u8) -> Result<Self, DecodeError> {
        let len = cache_len(bits)?;
        Ok(Self {
            bits,
            entries: vec![0; len].into_boxed_slice(),
        })
    }

    /// Returns the cache exponent declared by the VP8L stream.
    #[must_use]
    pub const fn bits(&self) -> u8 {
        self.bits
    }

    /// Returns the number of cache entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns whether the cache contains no addressable entries.
    ///
    /// A valid VP8L cache is never empty; this method exists for collection
    /// ergonomics and is always `false` for values constructed by [`Self::new`].
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Computes the VP8L cache index for `color` using the configured exponent.
    #[must_use]
    pub fn index_of(&self, color: u32) -> usize {
        // Construction validated `bits`, so the shift is always in 21..=31.
        hash_color(color, self.bits)
    }

    /// Retrieves the color at a bitstream-provided cache `index`.
    pub fn get(&self, index: usize) -> Result<u32, DecodeError> {
        self.entries.get(index).copied().ok_or_else(|| {
            DecodeError::new(
                DecodeErrorKind::InvalidBitstream,
                None,
                "VP8L color-cache index exceeds cache size",
            )
        })
    }

    /// Inserts `color`, overwriting any color that hashes to the same slot.
    ///
    /// Returns the slot selected by VP8L's multiplicative hash.
    pub fn insert(&mut self, color: u32) -> usize {
        let index = self.index_of(color);
        self.entries[index] = color;
        index
    }
}

/// Computes the VP8L cache index for a packed 32-bit color and cache exponent.
///
/// This is exposed separately for entropy decoders that validate stream state
/// before allocating the cache.  `color` is deliberately not decomposed into
/// channels: the full 32-bit value (including transparent RGB) participates in
/// the hash.
pub fn color_cache_index(color: u32, bits: u8) -> Result<usize, DecodeError> {
    cache_len(bits)?;
    Ok(hash_color(color, bits))
}

fn cache_len(bits: u8) -> Result<usize, DecodeError> {
    if !(MIN_COLOR_CACHE_BITS..=MAX_COLOR_CACHE_BITS).contains(&bits) {
        return Err(DecodeError::new(
            DecodeErrorKind::InvalidBitstream,
            None,
            "VP8L color-cache bits must be in 1..=11",
        ));
    }
    Ok(1_usize << bits)
}

fn hash_color(color: u32, bits: u8) -> usize {
    let shift = u32::BITS - u32::from(bits);
    (color.wrapping_mul(COLOR_CACHE_HASH_MULTIPLIER) >> shift) as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_every_valid_size_and_rejects_every_invalid_wire_value() {
        for bits in MIN_COLOR_CACHE_BITS..=MAX_COLOR_CACHE_BITS {
            let cache = ColorCache::new(bits).unwrap();
            assert_eq!(cache.bits(), bits);
            assert_eq!(cache.len(), 1_usize << bits);
            assert_eq!(cache.get(0).unwrap(), 0);
            assert_eq!(cache.get(cache.len() - 1).unwrap(), 0);
        }

        for bits in [0, 12, 13, 14, 15] {
            let error = ColorCache::new(bits).unwrap_err();
            assert_eq!(error.kind(), DecodeErrorKind::InvalidBitstream);
            assert!(color_cache_index(0, bits).is_err());
        }
    }

    #[test]
    fn hash_uses_wrapping_product_and_high_bits() {
        let cases = [
            (1, 0x0000_0000, 0),
            (1, 0xffff_ffff, 1),
            (2, 0x1234_5678, 2),
            (4, 0x1234_5678, 8),
            (8, 0x1234_5678, 0x8b),
            (11, 0x1234_5678, 0x45f),
        ];
        for (bits, color, expected) in cases {
            assert_eq!(color_cache_index(color, bits).unwrap(), expected);
        }
    }

    #[test]
    fn colliding_insert_overwrites_previous_color() {
        let mut cache = ColorCache::new(1).unwrap();
        let first = 0x0000_0001;
        let second = (0_u32..)
            .find(|&color| color != first && cache.index_of(color) == cache.index_of(first))
            .unwrap();

        let index = cache.insert(first);
        assert_eq!(cache.get(index).unwrap(), first);
        assert_eq!(cache.insert(second), index);
        assert_eq!(cache.get(index).unwrap(), second);
        assert!(cache.get(cache.len()).is_err());
    }

    #[test]
    fn transparent_pixels_hash_as_full_32_bit_colors() {
        let cache = ColorCache::new(11).unwrap();
        let transparent_black = 0x0000_0000;
        let transparent_rgb = 0x00ff_ffff;
        assert_ne!(
            cache.index_of(transparent_black),
            cache.index_of(transparent_rgb)
        );
    }

    #[test]
    fn exhaustive_small_sequences_match_direct_array_model() {
        // Exercise every short sequence over a small color alphabet for every
        // small cache size.  The direct model intentionally repeats the VP8L
        // hash expression instead of calling the implementation's helper.
        let colors: [u32; 4] = [0x0000_0000, 0x0000_0001, 0x00ff_ffff, 0x7f12_3456];
        for bits in 1..=4 {
            for first in colors {
                for second in colors {
                    for third in colors {
                        let mut cache = ColorCache::new(bits).unwrap();
                        let mut model = vec![0_u32; 1_usize << bits];
                        for color in [first, second, third] {
                            let index = ((color.wrapping_mul(COLOR_CACHE_HASH_MULTIPLIER))
                                >> (u32::BITS - u32::from(bits)))
                                as usize;
                            assert_eq!(cache.insert(color), index);
                            model[index] = color;
                        }
                        for (index, &expected) in model.iter().enumerate() {
                            assert_eq!(cache.get(index).unwrap(), expected);
                        }
                    }
                }
            }
        }
    }

    fn model_index(color: u32, bits: u8) -> usize {
        ((color.wrapping_mul(COLOR_CACHE_HASH_MULTIPLIER)) >> (u32::BITS - u32::from(bits)))
            as usize
    }

    fn assert_cache_matches_model(cache: &ColorCache, model: &[u32]) {
        assert_eq!(cache.len(), model.len());
        for (index, &expected) in model.iter().enumerate() {
            assert_eq!(cache.get(index), Ok(expected), "cache index {index}");
        }
    }

    fn model_emit(pixels: &mut Vec<u32>, cache: &mut [u32], bits: u8, color: u32) {
        pixels.push(color);
        cache[model_index(color, bits)] = color;
    }

    #[test]
    fn output_sink_matches_model_for_cache_hits_and_overlapping_copies() {
        // Use colliding colors so every emitted pixel must be inserted in the
        // correct order; the backwards copy alternates which color owns the
        // shared slot while it expands through its own output.
        let bits = 1;
        let first = 0x0102_0304;
        let shared_slot = model_index(first, bits);
        let second = (0_u32..)
            .find(|&color| color != first && model_index(color, bits) == shared_slot)
            .unwrap();

        let mut sink = ColorCacheOutput::with_cache_bits(bits).unwrap();
        let mut model_pixels = Vec::new();
        let mut model_cache = vec![0; 1_usize << bits];

        for color in [first, second, first] {
            sink.emit_literal(color).unwrap();
            model_emit(&mut model_pixels, &mut model_cache, bits, color);
            assert_eq!(sink.pixels(), model_pixels);
            assert_cache_matches_model(sink.cache(), &model_cache);
        }

        assert_eq!(sink.emit_cache_hit(shared_slot), Ok(first));
        model_emit(&mut model_pixels, &mut model_cache, bits, first);
        assert_eq!(sink.pixels(), model_pixels);
        assert_cache_matches_model(sink.cache(), &model_cache);

        // The last three produced pixels are [second, first, first].  A
        // length-five, distance-three copy is therefore
        // [second, first, first, second, first], including overlap-created
        // source pixels. The model writes the cache after each such emission.
        let copy_length = 5;
        let distance = 3;
        let mut copy_cache_trace = Vec::new();
        for _ in 0..copy_length {
            let color = model_pixels[model_pixels.len() - distance];
            model_emit(&mut model_pixels, &mut model_cache, bits, color);
            copy_cache_trace.push(model_cache[shared_slot]);
        }
        assert_eq!(copy_cache_trace, [second, first, first, second, first]);

        let mut budget = WorkBudget::new(copy_length as u64);
        sink.copy_lz77(copy_length, distance, model_pixels.len(), &mut budget)
            .unwrap();
        assert_eq!(sink.pixels(), model_pixels);
        assert_cache_matches_model(sink.cache(), &model_cache);
        assert_eq!(budget.remaining(), 0);
    }

    #[test]
    fn output_sink_rejects_bad_cache_indices_without_mutation() {
        let mut sink = ColorCacheOutput::with_cache_bits(2).unwrap();
        sink.emit_literal(0x1122_3344).unwrap();
        let before = sink.clone();

        let error = sink.emit_cache_hit(sink.cache().len()).unwrap_err();
        assert_eq!(error.kind(), DecodeErrorKind::InvalidBitstream);
        assert_eq!(sink, before);
    }

    #[test]
    fn failed_backward_reference_leaves_output_and_cache_unchanged() {
        let mut sink = ColorCacheOutput::with_cache_bits(2).unwrap();
        sink.emit_literal(0x1122_3344).unwrap();
        let before = sink.clone();
        let mut budget = WorkBudget::new(0);

        assert!(sink.copy_lz77(1, 1, 2, &mut budget).is_err());
        assert_eq!(sink, before);
    }
}
