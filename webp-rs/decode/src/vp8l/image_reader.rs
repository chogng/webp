use crate::BitReader;
use crate::DecodeError;
use crate::DecodeErrorKind;
use crate::DecodeLimits;
use crate::checked_image_bytes;
use crate::vp8l::header::HEADER_LEN;
use crate::vp8l::header::Vp8lHeader;
use crate::vp8l::header::parse_header;
use crate::vp8l::image_stream::decode_plan::{DecodePlan, KernelFamily};
use crate::vp8l::image_stream::symbol_stream::decode_image_data_rgba;
use crate::vp8l::image_stream::transform_list::DecodedTransform;
use crate::vp8l::image_stream::transform_list::read_transform_list;
use crate::vp8l::transforms::inverse_color::{inverse_color_rgba, inverse_subtract_green_rgba};
use crate::vp8l::transforms::inverse_indexing::inverse_color_indexing_rgba;
use crate::vp8l::transforms::inverse_predictor::inverse_predictor_rgba;

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
/// Transform subimages use packed `0xAARRGGBB` table storage. The main image
/// is entropy-expanded and inverse-transformed in one final-order RGBA byte
/// backing.
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
    let plan = DecodePlan::build(header, decoded_transforms, retained_transform_bytes, limits)?;
    debug_assert_eq!(plan.initial_work_units(), limits.max_work_units);
    debug_assert_eq!(plan.storage().full_image_allocations, 1);
    debug_assert_eq!(plan.storage().full_image_copy_bytes, 0);
    debug_assert_eq!(plan.storage().peak_image_backing_bytes, rgba_len);
    if plan.kernel() != KernelFamily::ScalarRgba {
        return Err(DecodeError::new(
            DecodeErrorKind::InvalidBitstream,
            None,
            "VP8L decode plan selected an unsupported kernel",
        ));
    }
    #[cfg(test)]
    let entropy_started = std::time::Instant::now();
    #[cfg(test)]
    if timings.is_some() {
        reset_entropy_path_counters();
    }
    let mut output = decode_image_data_rgba(
        &mut bits,
        plan.coded_width(),
        plan.coded_height(),
        &mut budget,
        limits,
        plan.retained_transform_bytes(),
        plan.rgba_len(),
    )?;
    if output.len() / 4 != plan.coded_pixels() {
        return Err(DecodeError::new(
            DecodeErrorKind::InvalidBitstream,
            None,
            "VP8L entropy output does not match planned coded geometry",
        ));
    }
    #[cfg(test)]
    if let Some(timings) = timings.as_mut() {
        timings.entropy += entropy_started.elapsed();
        timings.entropy_paths.add_assign(entropy_path_counters());
    }
    for transform in plan.transforms().iter().rev() {
        match transform {
            DecodedTransform::SubtractGreen => inverse_subtract_green_rgba(&mut output),
            DecodedTransform::Predictor {
                descriptor,
                mode_pixels,
            } => {
                #[cfg(test)]
                let predictor_started = std::time::Instant::now();
                inverse_predictor_rgba(&mut output, *descriptor, mode_pixels)?;
                #[cfg(test)]
                if let Some(timings) = timings.as_mut() {
                    timings.predictor += predictor_started.elapsed();
                }
            }
            DecodedTransform::Color {
                descriptor,
                multipliers,
            } => inverse_color_rgba(&mut output, *descriptor, multipliers)?,
            DecodedTransform::ColorIndexing {
                descriptor,
                palette,
            } => inverse_color_indexing_rgba(
                &mut output,
                *descriptor,
                palette,
                plan.retained_transform_bytes(),
                plan.rgba_len(),
                plan.max_alloc_bytes(),
            )?,
        }
    }
    let (header, rgba) = plan.finish(output)?;
    Ok(LiteralImage { header, rgba })
}

#[cfg(test)]
#[path = "image_reader_tests.rs"]
mod tests;
