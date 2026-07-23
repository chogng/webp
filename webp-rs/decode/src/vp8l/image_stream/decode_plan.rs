//! Validated ownership plan for one main-level VP8L decode.

use crate::DecodeError;
use crate::DecodeErrorKind;
use crate::DecodeLimits;
use crate::checked_image_bytes;
use crate::vp8l::allocation::pixel_count;
use crate::vp8l::header::Vp8lHeader;
use crate::vp8l::image_stream::transform_list::{DecodedTransform, DecodedTransformList};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::vp8l) enum KernelFamily {
    ScalarRgba,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::vp8l) struct DecodeStorageCensus {
    pub(in crate::vp8l) full_image_allocations: usize,
    pub(in crate::vp8l) full_image_copy_bytes: usize,
    pub(in crate::vp8l) peak_image_backing_bytes: usize,
}

pub(in crate::vp8l) struct DecodePlan {
    header: Vp8lHeader,
    transforms: Vec<DecodedTransform>,
    coded_width: u32,
    coded_height: u32,
    coded_pixels: usize,
    rgba_len: usize,
    retained_transform_bytes: usize,
    max_alloc_bytes: usize,
    initial_work_units: u64,
    kernel: KernelFamily,
    storage: DecodeStorageCensus,
}

impl DecodePlan {
    pub(in crate::vp8l) fn build(
        header: Vp8lHeader,
        decoded: DecodedTransformList,
        retained_transform_bytes: usize,
        limits: &DecodeLimits,
    ) -> Result<Self, DecodeError> {
        let rgba_len = checked_image_bytes(header.width, header.height, 4)?;
        if rgba_len > limits.max_alloc_bytes {
            return Err(DecodeError::new(
                DecodeErrorKind::LimitExceeded,
                None,
                "RGBA output exceeds configured allocation limit",
            ));
        }
        let coded_pixels = pixel_count(decoded.coded_width, decoded.coded_height)?;
        let final_pixels = pixel_count(header.width, header.height)?;
        if coded_pixels > final_pixels {
            return Err(DecodeError::new(
                DecodeErrorKind::InvalidBitstream,
                None,
                "VP8L coded image exceeds final image geometry",
            ));
        }

        validate_transform_pipeline(
            decoded.coded_width,
            decoded.coded_height,
            header,
            &decoded.transforms,
        )?;
        Ok(Self {
            header,
            transforms: decoded.transforms,
            coded_width: decoded.coded_width,
            coded_height: decoded.coded_height,
            coded_pixels,
            rgba_len,
            retained_transform_bytes,
            max_alloc_bytes: limits.max_alloc_bytes,
            initial_work_units: limits.max_work_units,
            kernel: KernelFamily::ScalarRgba,
            storage: DecodeStorageCensus {
                full_image_allocations: 1,
                full_image_copy_bytes: 0,
                peak_image_backing_bytes: rgba_len,
            },
        })
    }

    pub(in crate::vp8l) const fn coded_width(&self) -> u32 {
        self.coded_width
    }

    pub(in crate::vp8l) const fn coded_height(&self) -> u32 {
        self.coded_height
    }

    pub(in crate::vp8l) const fn coded_pixels(&self) -> usize {
        self.coded_pixels
    }

    pub(in crate::vp8l) const fn rgba_len(&self) -> usize {
        self.rgba_len
    }

    pub(in crate::vp8l) const fn retained_transform_bytes(&self) -> usize {
        self.retained_transform_bytes
    }

    pub(in crate::vp8l) const fn max_alloc_bytes(&self) -> usize {
        self.max_alloc_bytes
    }

    pub(in crate::vp8l) const fn initial_work_units(&self) -> u64 {
        self.initial_work_units
    }

    pub(in crate::vp8l) const fn kernel(&self) -> KernelFamily {
        self.kernel
    }

    pub(in crate::vp8l) fn transforms(&self) -> &[DecodedTransform] {
        &self.transforms
    }

    pub(in crate::vp8l) const fn storage(&self) -> DecodeStorageCensus {
        self.storage
    }

    pub(in crate::vp8l) fn finish(
        self,
        rgba: Vec<u8>,
    ) -> Result<(Vp8lHeader, Vec<u8>), DecodeError> {
        if rgba.len() != self.rgba_len {
            return Err(DecodeError::new(
                DecodeErrorKind::InvalidBitstream,
                None,
                "VP8L final RGBA buffer has unexpected length",
            ));
        }
        Ok((self.header, rgba))
    }
}

fn validate_transform_pipeline(
    mut width: u32,
    mut height: u32,
    header: Vp8lHeader,
    transforms: &[DecodedTransform],
) -> Result<(), DecodeError> {
    for transform in transforms.iter().rev() {
        match transform {
            DecodedTransform::SubtractGreen => {}
            DecodedTransform::Predictor { descriptor, .. }
            | DecodedTransform::Color { descriptor, .. } => {
                if descriptor.image_width != width || descriptor.image_height != height {
                    return Err(DecodeError::new(
                        DecodeErrorKind::InvalidBitstream,
                        None,
                        "VP8L transform pipeline geometry is inconsistent",
                    ));
                }
            }
            DecodedTransform::ColorIndexing { descriptor, .. } => {
                if descriptor.image_width_after != width || descriptor.image_height != height {
                    return Err(DecodeError::new(
                        DecodeErrorKind::InvalidBitstream,
                        None,
                        "VP8L color-indexing pipeline geometry is inconsistent",
                    ));
                }
                width = descriptor.image_width_before;
                height = descriptor.image_height;
            }
        }
    }
    if width != header.width || height != header.height {
        return Err(DecodeError::new(
            DecodeErrorKind::InvalidBitstream,
            None,
            "VP8L transform pipeline does not reach final image geometry",
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "decode_plan_tests.rs"]
mod tests;
