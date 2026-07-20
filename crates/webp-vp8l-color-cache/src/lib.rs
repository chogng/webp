#![forbid(unsafe_code)]
//! The VP8L color cache.
//!
//! VP8L hashes complete packed 32-bit colors, including alpha and RGB values
//! of fully transparent pixels.  A cache slot is overwritten every time a
//! decoded pixel is inserted; callers must therefore call [`ColorCache::insert`]
//! for literals, cache hits, and every pixel emitted by a backward reference.

use webp_core::{DecodeError, DecodeErrorKind};

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
}
