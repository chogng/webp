//! Entropy/LZ77 output backed by one layout-specific pixel allocation.

use crate::DecodeError;
use crate::DecodeErrorKind;
use crate::WorkBudget;
use crate::vp8l::backward_references::{
    copy_lz77_pixels_preallocated, copy_lz77_rgba_preallocated,
};
use crate::vp8l::color_cache::ColorCache;
use crate::vp8l::pixel::{pack_argb, unpack_rgba};

pub(in crate::vp8l) enum PixelBacking {
    PackedArgb(Vec<u32>),
    Rgba(Vec<u8>),
}

pub(in crate::vp8l) struct PixelOutput {
    backing: PixelBacking,
    cache: Option<DeferredColorCache>,
}

struct DeferredColorCache {
    cache: ColorCache,
    cached_pixels: usize,
}

impl PixelOutput {
    pub(in crate::vp8l) fn new_packed(
        color_cache_bits: Option<u8>,
        pixels: usize,
    ) -> Result<Self, DecodeError> {
        let mut output = Vec::new();
        output.try_reserve_exact(pixels).map_err(|_| {
            DecodeError::new(
                DecodeErrorKind::AllocationFailed,
                None,
                "packed VP8L output allocation failed",
            )
        })?;
        Self::with_backing(color_cache_bits, PixelBacking::PackedArgb(output))
    }

    pub(in crate::vp8l) fn new_rgba(
        color_cache_bits: Option<u8>,
        final_pixels: usize,
    ) -> Result<Self, DecodeError> {
        let capacity = final_pixels.checked_mul(4).ok_or_else(|| {
            DecodeError::new(
                DecodeErrorKind::LimitExceeded,
                None,
                "VP8L RGBA output byte size overflow",
            )
        })?;
        let mut output = Vec::new();
        output.try_reserve_exact(capacity).map_err(|_| {
            DecodeError::new(
                DecodeErrorKind::AllocationFailed,
                None,
                "RGBA output allocation failed",
            )
        })?;
        Self::with_backing(color_cache_bits, PixelBacking::Rgba(output))
    }

    fn with_backing(
        color_cache_bits: Option<u8>,
        backing: PixelBacking,
    ) -> Result<Self, DecodeError> {
        let cache = color_cache_bits
            .map(|bits| {
                Ok(DeferredColorCache {
                    cache: ColorCache::new(bits)?,
                    cached_pixels: 0,
                })
            })
            .transpose()?;
        Ok(Self { backing, cache })
    }

    pub(in crate::vp8l) fn len(&self) -> usize {
        match &self.backing {
            PixelBacking::PackedArgb(pixels) => pixels.len(),
            PixelBacking::Rgba(bytes) => bytes.len() / 4,
        }
    }

    pub(in crate::vp8l) fn emit_literal(&mut self, color: u32) -> Result<(), DecodeError> {
        match &mut self.backing {
            PixelBacking::PackedArgb(pixels) => pixels.push(color),
            PixelBacking::Rgba(bytes) => bytes.extend_from_slice(&unpack_rgba(color)),
        }
        Ok(())
    }

    pub(in crate::vp8l) fn emit_cache_hit(&mut self, index: usize) -> Result<(), DecodeError> {
        let produced_pixels = self.len();
        let deferred = self.cache.as_mut().ok_or_else(|| {
            DecodeError::new(
                DecodeErrorKind::InvalidBitstream,
                None,
                "VP8L color-cache symbol appeared without a color cache",
            )
        })?;
        match &self.backing {
            PixelBacking::PackedArgb(pixels) => {
                for &color in &pixels[deferred.cached_pixels..] {
                    deferred.cache.insert(color);
                }
            }
            PixelBacking::Rgba(bytes) => {
                for pixel in bytes[deferred.cached_pixels * 4..].chunks_exact(4) {
                    deferred
                        .cache
                        .insert(pack_argb(pixel[0], pixel[1], pixel[2], pixel[3]));
                }
            }
        }
        deferred.cached_pixels = produced_pixels;
        let color = deferred.cache.get(index)?;
        self.emit_literal(color)
    }

    pub(in crate::vp8l) fn copy_lz77(
        &mut self,
        length: usize,
        distance: usize,
        output_limit: usize,
        budget: &mut WorkBudget,
    ) -> Result<(), DecodeError> {
        match &mut self.backing {
            PixelBacking::PackedArgb(pixels) => {
                copy_lz77_pixels_preallocated(pixels, length, distance, output_limit, budget)
            }
            PixelBacking::Rgba(bytes) => {
                copy_lz77_rgba_preallocated(bytes, length, distance, output_limit, budget)
            }
        }
    }

    pub(in crate::vp8l) fn into_pixels(self) -> Result<Vec<u32>, DecodeError> {
        match self.backing {
            PixelBacking::PackedArgb(pixels) => Ok(pixels),
            PixelBacking::Rgba(_) => Err(DecodeError::new(
                DecodeErrorKind::InvalidBitstream,
                None,
                "VP8L packed subimage used an RGBA output backing",
            )),
        }
    }

    pub(in crate::vp8l) fn into_rgba(self) -> Result<Vec<u8>, DecodeError> {
        match self.backing {
            PixelBacking::Rgba(bytes) => Ok(bytes),
            PixelBacking::PackedArgb(_) => Err(DecodeError::new(
                DecodeErrorKind::InvalidBitstream,
                None,
                "VP8L main image used a packed output backing",
            )),
        }
    }
}
