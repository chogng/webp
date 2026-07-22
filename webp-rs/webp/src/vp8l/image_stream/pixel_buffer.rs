use crate::vp8l::header::BlockTransformDescriptor;
use crate::vp8l::pixel::extend_rgba_from_argb;
use crate::vp8l::pixel::pack_argb;
use crate::vp8l::transforms::inverse_predictor::inverse_predictor_argb_to_rgba;
use crate::vp8l::transforms::inverse_predictor::inverse_predictor_rgba;
use webp_core::DecodeError;
use webp_core::DecodeErrorKind;

/// Internal transform storage with explicit intermediate layout states.
///
/// Entropy and color indexing naturally operate on VP8L's packed ARGB words,
/// while predictor reconstruction benefits from channel-contiguous RGBA byte
/// lanes. Keeping the conversion at this private boundary leaves the public
/// transform crate and its other callers unchanged.
pub(in crate::vp8l) enum PixelBuffer {
    Argb(Vec<u32>),
    Rgba(Vec<u8>),
}

impl PixelBuffer {
    pub(in crate::vp8l) fn argb_mut(&mut self) -> Result<&mut Vec<u32>, DecodeError> {
        if let Self::Rgba(bytes) = self {
            if !bytes.len().is_multiple_of(4) {
                return Err(DecodeError::new(
                    DecodeErrorKind::InvalidBitstream,
                    None,
                    "VP8L RGBA transform buffer has unexpected length",
                ));
            }
            let mut packed = Vec::new();
            packed.try_reserve_exact(bytes.len() / 4).map_err(|_| {
                DecodeError::new(
                    DecodeErrorKind::AllocationFailed,
                    None,
                    "VP8L packed transform allocation failed",
                )
            })?;
            for pixel in bytes.chunks_exact(4) {
                packed.push(pack_argb(pixel[0], pixel[1], pixel[2], pixel[3]));
            }
            *self = Self::Argb(packed);
        }
        match self {
            Self::Argb(pixels) => Ok(pixels),
            Self::Rgba(_) => unreachable!("RGBA transform buffer was converted to ARGB"),
        }
    }

    fn rgba_mut(&mut self) -> Result<&mut Vec<u8>, DecodeError> {
        if let Self::Argb(pixels) = self {
            let actual_len = pixels.len().checked_mul(4).ok_or_else(|| {
                DecodeError::new(
                    DecodeErrorKind::LimitExceeded,
                    None,
                    "VP8L RGBA transform length overflow",
                )
            })?;
            let mut bytes = Vec::new();
            bytes.try_reserve_exact(actual_len).map_err(|_| {
                DecodeError::new(
                    DecodeErrorKind::AllocationFailed,
                    None,
                    "RGBA output allocation failed",
                )
            })?;
            extend_rgba_from_argb(&mut bytes, pixels);
            *self = Self::Rgba(bytes);
        }
        match self {
            Self::Rgba(bytes) => Ok(bytes),
            Self::Argb(_) => unreachable!("ARGB transform buffer was converted to RGBA"),
        }
    }

    pub(in crate::vp8l) fn inverse_predictor(
        &mut self,
        descriptor: BlockTransformDescriptor,
        mode_pixels: &[u32],
    ) -> Result<(), DecodeError> {
        let converted = match self {
            Self::Argb(pixels) => Some(inverse_predictor_argb_to_rgba(
                pixels,
                descriptor,
                mode_pixels,
            )?),
            Self::Rgba(bytes) => {
                inverse_predictor_rgba(bytes, descriptor, mode_pixels)?;
                None
            }
        };
        if let Some(bytes) = converted {
            *self = Self::Rgba(bytes);
        }
        Ok(())
    }

    pub(in crate::vp8l) fn into_rgba(
        mut self,
        expected_rgba_len: usize,
    ) -> Result<Vec<u8>, DecodeError> {
        self.rgba_mut()?;
        match self {
            Self::Rgba(bytes) if bytes.len() == expected_rgba_len => Ok(bytes),
            Self::Rgba(_) => Err(DecodeError::new(
                DecodeErrorKind::InvalidBitstream,
                None,
                "VP8L final RGBA buffer has unexpected length",
            )),
            Self::Argb(_) => unreachable!("ARGB transform buffer was converted to RGBA"),
        }
    }
}
