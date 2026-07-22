use crate::vp8l::header::HEADER_LEN;
use crate::vp8l::header::Vp8lHeader;
use crate::vp8l::header::parse_header;
use crate::vp8l::image_stream::pixel_buffer::PixelBuffer;
use crate::vp8l::image_stream::symbol_stream::decode_image_data;
use crate::vp8l::image_stream::transform_list::DecodedTransform;
use crate::vp8l::image_stream::transform_list::read_transform_list;
use crate::vp8l::transforms::inverse_color::inverse_color_argb;
use crate::vp8l::transforms::inverse_color::inverse_subtract_green_argb;
use crate::vp8l::transforms::inverse_indexing::inverse_color_indexing_argb;
use webp_core::BitReader;
use webp_core::DecodeError;
use webp_core::DecodeErrorKind;
use webp_core::DecodeLimits;
use webp_core::checked_image_bytes;

#[cfg(test)]
use crate::vp8l::image_stream::decode_profile::DecodePhaseTimings;
#[cfg(test)]
use crate::vp8l::image_stream::decode_profile::entropy_path_counters;
#[cfg(test)]
use crate::vp8l::image_stream::decode_profile::reset_entropy_path_counters;

/// A decoded straight/unpremultiplied RGBA image.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiteralImage {
    /// Fixed VP8L image information.
    pub header: Vp8lHeader,
    /// Pixels in row-major RGBA8 byte order.
    pub rgba: Vec<u8>,
}

#[cfg(test)]
#[path = "predictor_benchmark_tests.rs"]
mod predictor_benchmark_tests;

/// Decodes a standalone static VP8L stream to straight RGBA8.
///
/// The input begins with the five-byte VP8L fixed header.
pub fn decode_vp8l(data: &[u8], limits: &DecodeLimits) -> Result<LiteralImage, DecodeError> {
    decode_no_transform(data, limits)
}

/// Backwards-compatible name for [`decode_vp8l`].
pub fn decode_literal_only(
    data: &[u8],
    limits: &DecodeLimits,
) -> Result<LiteralImage, DecodeError> {
    decode_vp8l(data, limits)
}

/// Decodes a standalone static VP8L stream.
///
/// Literal pixels, green-alphabet backward-reference symbols, and color-cache
/// references are supported. The transform list may be empty or contain
/// subtract-green, predictor, color, and color-indexing transforms. Main
/// images may use spatial meta-Huffman groups; transform subimages cannot.
/// Internally decoded samples are packed as `0xAARRGGBB` until entropy
/// expansion is complete, then inverse-transformed and emitted in RGBA byte
/// order.
pub fn decode_no_transform(
    data: &[u8],
    limits: &DecodeLimits,
) -> Result<LiteralImage, DecodeError> {
    decode_no_transform_inner(
        data,
        limits,
        #[cfg(test)]
        None,
    )
}

#[cfg(test)]
fn decode_no_transform_profiled(
    data: &[u8],
    limits: &DecodeLimits,
    timings: &mut DecodePhaseTimings,
) -> Result<LiteralImage, DecodeError> {
    decode_no_transform_inner(data, limits, Some(timings))
}

fn decode_no_transform_inner(
    data: &[u8],
    limits: &DecodeLimits,
    #[cfg(test)] mut timings: Option<&mut DecodePhaseTimings>,
) -> Result<LiteralImage, DecodeError> {
    let header = parse_header(data, limits)?;
    let rgba_len = checked_image_bytes(header.width, header.height, 4)?;
    if rgba_len > limits.max_alloc_bytes {
        return Err(DecodeError::new(
            DecodeErrorKind::LimitExceeded,
            None,
            "RGBA output exceeds configured allocation limit",
        ));
    }

    let mut bits = BitReader::with_bit_position(data, HEADER_LEN * 8)?;
    let mut budget = limits.work_budget();

    let mut retained_transform_bytes = 0_usize;
    let decoded_transforms = read_transform_list(
        &mut bits,
        &mut budget,
        &header,
        limits,
        &mut retained_transform_bytes,
    )?;
    #[cfg(test)]
    let entropy_started = std::time::Instant::now();
    #[cfg(test)]
    if timings.is_some() {
        reset_entropy_path_counters();
    }
    let output = decode_image_data(
        &mut bits,
        decoded_transforms.coded_width,
        decoded_transforms.coded_height,
        true,
        &mut budget,
        limits,
        retained_transform_bytes,
        rgba_len,
    )?;
    #[cfg(test)]
    if let Some(timings) = timings.as_mut() {
        timings.entropy += entropy_started.elapsed();
        timings.entropy_paths.add_assign(entropy_path_counters());
    }
    let mut output = PixelBuffer::Argb(output);

    for transform in decoded_transforms.transforms.iter().rev() {
        match transform {
            DecodedTransform::SubtractGreen => inverse_subtract_green_argb(output.argb_mut()?),
            DecodedTransform::Predictor {
                descriptor,
                mode_pixels,
            } => {
                #[cfg(test)]
                let predictor_started = std::time::Instant::now();
                output.inverse_predictor(*descriptor, mode_pixels)?;
                #[cfg(test)]
                if let Some(timings) = timings.as_mut() {
                    timings.predictor += predictor_started.elapsed();
                }
            }
            DecodedTransform::Color {
                descriptor,
                multipliers,
            } => inverse_color_argb(output.argb_mut()?, *descriptor, multipliers)?,
            DecodedTransform::ColorIndexing {
                descriptor,
                palette,
            } => inverse_color_indexing_argb(
                output.argb_mut()?,
                *descriptor,
                palette,
                retained_transform_bytes,
                rgba_len,
                limits.max_alloc_bytes,
            )?,
        }
    }
    drop(decoded_transforms);
    #[cfg(test)]
    let conversion_started = std::time::Instant::now();
    let rgba = output.into_rgba(rgba_len)?;
    #[cfg(test)]
    if let Some(timings) = timings.as_mut() {
        timings.rgba_conversion += conversion_started.elapsed();
    }

    Ok(LiteralImage { header, rgba })
}

#[cfg(test)]
#[path = "image_reader_tests.rs"]
mod tests;
