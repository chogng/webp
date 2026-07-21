use webp_core::DecodeError;
use webp_core::DecodeErrorKind;
use webp_core::WorkBudget;
use webp_vp8l_color_cache::ColorCache;
use webp_vp8l_entropy::copy_lz77_pixels_preallocated;

pub(super) struct PixelOutput {
    pixels: Vec<u32>,
    cache: Option<DeferredColorCache>,
}

struct DeferredColorCache {
    cache: ColorCache,
    cached_pixels: usize,
}

impl PixelOutput {
    pub(super) fn new(color_cache_bits: Option<u8>, pixels: usize) -> Result<Self, DecodeError> {
        let mut output = Vec::new();
        output.try_reserve_exact(pixels).map_err(|_| {
            DecodeError::new(
                DecodeErrorKind::AllocationFailed,
                None,
                "packed VP8L output allocation failed",
            )
        })?;
        let cache = color_cache_bits
            .map(|bits| {
                Ok(DeferredColorCache {
                    cache: ColorCache::new(bits)?,
                    cached_pixels: 0,
                })
            })
            .transpose()?;
        Ok(Self {
            pixels: output,
            cache,
        })
    }

    pub(super) fn len(&self) -> usize {
        self.pixels.len()
    }

    pub(super) fn emit_literal(&mut self, color: u32) -> Result<(), DecodeError> {
        // `PixelOutput::new` reserved the complete, already validated image
        // size. The enclosing entropy loop cannot emit past that size, so
        // this push never grows the vector. Cache population is deferred
        // until a cache symbol actually needs the state.
        self.pixels.push(color);
        Ok(())
    }

    pub(super) fn emit_cache_hit(&mut self, index: usize) -> Result<(), DecodeError> {
        let deferred = self.cache.as_mut().ok_or_else(|| {
            DecodeError::new(
                DecodeErrorKind::InvalidBitstream,
                None,
                "VP8L color-cache symbol appeared without a color cache",
            )
        })?;
        for &color in &self.pixels[deferred.cached_pixels..] {
            deferred.cache.insert(color);
        }
        deferred.cached_pixels = self.pixels.len();
        let color = deferred.cache.get(index)?;
        self.pixels.push(color);
        Ok(())
    }

    pub(super) fn copy_lz77(
        &mut self,
        length: usize,
        distance: usize,
        output_limit: usize,
        budget: &mut WorkBudget,
    ) -> Result<(), DecodeError> {
        copy_lz77_pixels_preallocated(&mut self.pixels, length, distance, output_limit, budget)
    }

    pub(super) fn into_pixels(self) -> Vec<u32> {
        self.pixels
    }
}
