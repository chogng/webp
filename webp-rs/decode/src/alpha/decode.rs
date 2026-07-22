//! Reads WebP `ALPH` chunk payloads into straight alpha samples.
//!
//! The chunk header describes an alpha-plane compression method and a
//! reversible spatial filter. Both raw alpha (method 0) and headerless VP8L
//! alpha (method 1) are recovered into a straight alpha plane.

use super::wire::AlphaCompression;
use super::wire::AlphaFilter;
use super::wire::AlphaHeader;
use super::wire::AlphaPreprocessing;
use crate::BitWriter;
use crate::CompatibilityProfile;
use crate::DecodeError;
use crate::DecodeErrorKind;
use crate::DecodeLimits;
use crate::checked_image_bytes;

/// Parses an `ALPH` header according to the selected compatibility profile.
pub fn parse_header(
    payload: &[u8],
    profile: CompatibilityProfile,
) -> Result<AlphaHeader, DecodeError> {
    let byte = *payload.first().ok_or_else(|| {
        DecodeError::new(
            DecodeErrorKind::UnexpectedEof,
            Some(0),
            "truncated ALPH header",
        )
    })?;
    if profile == CompatibilityProfile::SpecStrict && byte >> 6 != 0 {
        return Err(DecodeError::new(
            DecodeErrorKind::InvalidBitstream,
            Some(0),
            "ALPH reserved bits are non-zero",
        ));
    }
    let compression = match byte & 0b11 {
        0 => AlphaCompression::Raw,
        1 => AlphaCompression::Lossless,
        _ => {
            return Err(DecodeError::new(
                DecodeErrorKind::InvalidBitstream,
                Some(0),
                "unsupported ALPH compression method",
            ));
        }
    };
    let filter = match (byte >> 2) & 0b11 {
        0 => AlphaFilter::None,
        1 => AlphaFilter::Horizontal,
        2 => AlphaFilter::Vertical,
        3 => AlphaFilter::Gradient,
        _ => unreachable!("two-bit ALPH filter"),
    };
    let preprocessing = match (byte >> 4) & 0b11 {
        0 => AlphaPreprocessing::None,
        1 => AlphaPreprocessing::LevelReduction,
        _ => {
            return Err(DecodeError::new(
                DecodeErrorKind::InvalidBitstream,
                Some(0),
                "unsupported ALPH preprocessing method",
            ));
        }
    };
    Ok(AlphaHeader {
        compression,
        filter,
        preprocessing,
    })
}

/// Decodes an `ALPH` payload into row-major alpha bytes.
///
/// Raw alpha uses the reversible spatial filters directly. Lossless alpha
/// supplies a headerless VP8L entropy stream, whose decoded green channel is
/// the alpha plane.
pub fn decode(
    payload: &[u8],
    width: u32,
    height: u32,
    profile: CompatibilityProfile,
    limits: &DecodeLimits,
) -> Result<Vec<u8>, DecodeError> {
    if width == 0 || height == 0 {
        return Err(DecodeError::new(
            DecodeErrorKind::InvalidParameter,
            None,
            "ALPH dimensions must be non-zero",
        ));
    }
    limits.check_input_len(payload.len())?;
    limits.check_image(width, height)?;
    let header = parse_header(payload, profile)?;
    let plane_len = checked_image_bytes(width, height, 1)?;
    if plane_len > limits.max_alloc_bytes {
        return Err(DecodeError::new(
            DecodeErrorKind::LimitExceeded,
            None,
            "alpha output exceeds configured allocation limit",
        ));
    }
    let samples = payload.get(1..).expect("ALPH header was present");
    match header.compression {
        AlphaCompression::Raw => {
            decode_raw_samples(samples, plane_len, width, header.filter, limits)
        }
        AlphaCompression::Lossless => {
            decode_lossless_samples(samples, plane_len, width, height, header.filter, limits)
        }
    }
}

/// Backwards-compatible raw-only entry point for callers that have already
/// selected compression method 0.
pub fn decode_raw(
    payload: &[u8],
    width: u32,
    height: u32,
    profile: CompatibilityProfile,
    limits: &DecodeLimits,
) -> Result<Vec<u8>, DecodeError> {
    let header = parse_header(payload, profile)?;
    if header.compression != AlphaCompression::Raw {
        return Err(DecodeError::new(
            DecodeErrorKind::InvalidParameter,
            Some(0),
            "decode_raw requires ALPH compression method 0",
        ));
    }
    decode(payload, width, height, profile, limits)
}

fn decode_raw_samples(
    samples: &[u8],
    plane_len: usize,
    width: u32,
    filter: AlphaFilter,
    limits: &DecodeLimits,
) -> Result<Vec<u8>, DecodeError> {
    if samples.len() < plane_len {
        return Err(DecodeError::new(
            DecodeErrorKind::UnexpectedEof,
            Some(samples.len() + 1),
            "truncated raw ALPH payload",
        ));
    }
    if samples.len() > plane_len {
        return Err(DecodeError::new(
            DecodeErrorKind::InvalidBitstream,
            Some(plane_len + 1),
            "raw ALPH payload has trailing bytes",
        ));
    }

    let mut output = Vec::new();
    output.try_reserve_exact(plane_len).map_err(|_| {
        DecodeError::new(
            DecodeErrorKind::AllocationFailed,
            None,
            "cannot reserve alpha output",
        )
    })?;
    output.resize(plane_len, 0);
    let width = usize::try_from(width).map_err(|_| {
        DecodeError::new(
            DecodeErrorKind::InvalidParameter,
            None,
            "alpha width exceeds usize",
        )
    })?;
    output.copy_from_slice(samples);
    unfilter_in_place(&mut output, width, filter, limits)?;
    Ok(output)
}

fn unfilter_in_place(
    output: &mut [u8],
    width: usize,
    filter: AlphaFilter,
    limits: &DecodeLimits,
) -> Result<(), DecodeError> {
    let mut budget = limits.work_budget();
    for index in 0..output.len() {
        budget.consume(1)?;
        let x = index % width;
        let y = index / width;
        let left = if x != 0 { output[index - 1] } else { 0 };
        let top = if y != 0 { output[index - width] } else { 0 };
        let top_left = if x != 0 && y != 0 {
            output[index - width - 1]
        } else {
            0
        };
        let predictor = match filter {
            AlphaFilter::None => 0,
            AlphaFilter::Horizontal => {
                if x == 0 {
                    top
                } else {
                    left
                }
            }
            AlphaFilter::Vertical => {
                if y == 0 {
                    left
                } else {
                    top
                }
            }
            AlphaFilter::Gradient => {
                if x == 0 {
                    top
                } else if y == 0 {
                    left
                } else {
                    gradient(left, top, top_left)
                }
            }
        };
        output[index] = output[index].wrapping_add(predictor);
    }
    Ok(())
}

fn decode_lossless_samples(
    samples: &[u8],
    plane_len: usize,
    width: u32,
    height: u32,
    filter: AlphaFilter,
    limits: &DecodeLimits,
) -> Result<Vec<u8>, DecodeError> {
    let mut encoded = vp8l_header(width, height)?;
    encoded.try_reserve(samples.len()).map_err(|_| {
        DecodeError::new(
            DecodeErrorKind::AllocationFailed,
            None,
            "cannot reserve synthetic VP8L alpha input",
        )
    })?;
    encoded.extend_from_slice(samples);

    // The public input was already checked before the fixed five-byte VP8L
    // header was synthesized. The decoder's input limit must account for that
    // internal representation, without weakening its image/allocation limits.
    let mut vp8l_limits = limits.clone();
    vp8l_limits.max_input_bytes = limits
        .max_input_bytes
        .saturating_add(encoded.len() - samples.len());
    let image = crate::vp8l::image_reader::decode_vp8l(&encoded, &vp8l_limits)?;
    let mut alpha = Vec::new();
    alpha.try_reserve_exact(plane_len).map_err(|_| {
        DecodeError::new(
            DecodeErrorKind::AllocationFailed,
            None,
            "cannot reserve decoded lossless alpha output",
        )
    })?;
    for rgba in image.rgba.chunks_exact(4) {
        alpha.push(rgba[1]);
    }
    debug_assert_eq!(alpha.len(), plane_len);
    let width = usize::try_from(width).map_err(|_| {
        DecodeError::new(
            DecodeErrorKind::InvalidParameter,
            None,
            "alpha width exceeds usize",
        )
    })?;
    unfilter_in_place(&mut alpha, width, filter, limits)?;
    Ok(alpha)
}

fn vp8l_header(width: u32, height: u32) -> Result<Vec<u8>, DecodeError> {
    if width == 0 || height == 0 || width > (1 << 14) || height > (1 << 14) {
        return Err(DecodeError::new(
            DecodeErrorKind::InvalidParameter,
            None,
            "ALPH dimensions cannot be represented by VP8L",
        ));
    }
    let mut bits = BitWriter::new();
    let map_write_error = |_| {
        DecodeError::new(
            DecodeErrorKind::AllocationFailed,
            None,
            "cannot construct VP8L alpha header",
        )
    };
    bits.write_bits(0x2f, 8).map_err(map_write_error)?;
    bits.write_bits(width - 1, 14).map_err(map_write_error)?;
    bits.write_bits(height - 1, 14).map_err(map_write_error)?;
    bits.write_bits(0, 1).map_err(map_write_error)?; // alpha is represented by outer ALPH.
    bits.write_bits(0, 3).map_err(map_write_error)?; // VP8L version.
    Ok(bits.into_bytes())
}

#[inline]
fn gradient(left: u8, top: u8, top_left: u8) -> u8 {
    (left as i16 + top as i16 - top_left as i16).clamp(0, 255) as u8
}

#[cfg(test)]
#[path = "decode_tests.rs"]
mod tests;
