//! Public encode orchestration across container and codec owners.

use crate::EncodeError;
use crate::LosslessEncodeOptions;
use crate::LosslessEncodeProfile;
use crate::LossyEncodeOptions;
use crate::Metadata;
#[cfg(test)]
use crate::vp8l::header::MAX_DIMENSION;
#[cfg(test)]
use crate::vp8l::image_writer::COLOR_TRANSFORM_BLOCK_BITS;
#[cfg(test)]
use crate::vp8l::image_writer::EntropyToken;
#[cfg(test)]
use crate::vp8l::image_writer::collect_entropy_tokens;
use crate::vp8l::image_writer::encode_vp8l_payload;
#[cfg(test)]
use crate::vp8l::image_writer::select_color_cache_bits;
#[cfg(test)]
use crate::vp8l::image_writer::select_color_transform;
#[cfg(test)]
use crate::vp8l::image_writer::select_left_predictor;
use crate::vp8l::image_writer::spatial_plan;
use crate::vp8l::image_writer::spatial_writer;
#[cfg(test)]
use crate::vp8l::image_writer::try_make_palette_plan;
use crate::vp8l::image_writer::validate_input;

/// Encodes a static RGBA8 image as a lossless WebP file.
///
/// The input is straight/unpremultiplied RGBA in row-major order. This first
/// encoder slice always emits a static VP8L image with reversible
/// subtract-green and fixed left-predictor transforms, then literal pixels.
/// It is format-correct but does not attempt to optimize output size or speed.
///
/// # Errors
///
/// Returns [`EncodeError`] when dimensions are outside VP8L's representable
/// range, the byte slice does not exactly contain `width * height * 4` bytes,
/// or output allocation fails.
pub fn encode_lossless_rgba(width: u32, height: u32, rgba: &[u8]) -> Result<Vec<u8>, EncodeError> {
    let (payload, _) = encode_vp8l_payload(width, height, rgba)?;
    wrap_vp8l(payload)
}

/// Encodes a static RGBA8 image with an explicit lossless profile.
///
/// [`LosslessEncodeProfile::Default`] is byte-for-byte equivalent to
/// [`encode_lossless_rgba`]. Fast-decode profiles emit standard VP8L and use a
/// deterministic complete-file fallback when spatial Huffman groups do not
/// make the file strictly smaller than the corresponding fast-no-cache
/// single-group stream. They can be larger than the default profile and use
/// an exact same-profile single-stream cost before serializing only the
/// selected complete file.
///
/// ```
/// use webp::{
///     LosslessEncodeOptions, LosslessEncodeProfile, encode_lossless_rgba_with_options,
/// };
///
/// let rgba = [10, 20, 30, 255];
/// let mut options = LosslessEncodeOptions::default();
/// options.profile = LosslessEncodeProfile::FastDecodeLowLatency;
/// let encoded = encode_lossless_rgba_with_options(1, 1, &rgba, options)?;
/// assert_eq!(&encoded[..4], b"RIFF");
/// # Ok::<(), webp::EncodeError>(())
/// ```
///
/// # Errors
///
/// Returns the same errors as [`encode_lossless_rgba`].
pub fn encode_lossless_rgba_with_options(
    width: u32,
    height: u32,
    rgba: &[u8],
    options: LosslessEncodeOptions,
) -> Result<Vec<u8>, EncodeError> {
    match options.profile {
        LosslessEncodeProfile::Default => encode_lossless_rgba(width, height, rgba),
        LosslessEncodeProfile::FastDecodeCompact => spatial_writer::encode_profile(
            width,
            height,
            rgba,
            spatial_plan::SpatialProfile::Compact,
        ),
        LosslessEncodeProfile::FastDecodeLowLatency => spatial_writer::encode_profile(
            width,
            height,
            rgba,
            spatial_plan::SpatialProfile::LowLatency,
        ),
    }
}

/// Encodes an opaque RGBA8 image as a lossy VP8 WebP file.
///
/// This first public M7 profile uses DC intra prediction with quantized
/// residuals. Non-opaque alpha is serialized as a raw `ALPH` plane; metadata
/// and animation remain outside the current VP8 encoder profile.
pub fn encode_lossy_rgba(width: u32, height: u32, rgba: &[u8]) -> Result<Vec<u8>, EncodeError> {
    encode_lossy_rgba_with_options(width, height, rgba, LossyEncodeOptions::default())
}

/// Encodes the currently supported static lossy VP8 profile with explicit quality.
pub fn encode_lossy_rgba_with_options(
    width: u32,
    height: u32,
    rgba: &[u8],
    options: LossyEncodeOptions,
) -> Result<Vec<u8>, EncodeError> {
    encode_lossy_rgba_with_alpha_options(
        width,
        height,
        rgba,
        options,
        crate::alpha::AlphaEncodeOptions::default(),
    )
}

/// Encodes the lossy VP8 profile with an explicit `ALPH` payload policy.
///
/// The alpha policy is used only when the input contains a non-opaque sample.
pub fn encode_lossy_rgba_with_alpha_options(
    width: u32,
    height: u32,
    rgba: &[u8],
    options: LossyEncodeOptions,
    alpha_options: crate::alpha::AlphaEncodeOptions,
) -> Result<Vec<u8>, EncodeError> {
    if options.quality > 100 {
        return Err(EncodeError::invalid_quality());
    }
    validate_input(width, height, rgba)?;
    let has_alpha = rgba.chunks_exact(4).any(|pixel| pixel[3] != u8::MAX);
    let alpha = if has_alpha {
        let mut alpha = Vec::new();
        alpha
            .try_reserve_exact(rgba.len() / 4)
            .map_err(|_| EncodeError::allocation_failed())?;
        alpha.extend(rgba.chunks_exact(4).map(|pixel| pixel[3]));
        Some(alpha)
    } else {
        None
    };
    let source = crate::vp8::rgba_to_yuv420(width, height, rgba).map_err(map_vp8_encode_error)?;
    let quantizer = u8::try_from((u16::from(100 - options.quality) * 127) / 100)
        .map_err(|_| EncodeError::invalid_quality())?;
    let payload = crate::vp8::encode_dc_predicted_key_frame_with_quantizer(&source, quantizer)
        .map_err(map_vp8_encode_error)?;
    wrap_vp8(payload, width, height, alpha, alpha_options)
}

/// Encodes a static RGBA8 image as a lossless WebP file with raw metadata.
///
/// ICCP, EXIF, and XMP payloads are preserved byte-for-byte. When at least
/// one metadata payload is present, the returned strict container includes a
/// `VP8X` header with matching feature flags.
///
/// # Errors
///
/// Returns [`EncodeError`] for the same image and allocation failures as
/// [`encode_lossless_rgba`], or when the resulting RIFF/chunk sizes cannot be
/// represented by WebP's 32-bit length fields.
pub fn encode_lossless_rgba_with_metadata(
    width: u32,
    height: u32,
    rgba: &[u8],
    metadata: &Metadata,
) -> Result<Vec<u8>, EncodeError> {
    let (payload, has_alpha) = encode_vp8l_payload(width, height, rgba)?;
    wrap_vp8l_with_metadata(payload, width, height, has_alpha, metadata)
}

/// Encodes static RGBA8 with raw metadata and an explicit lossless profile.
///
/// Metadata payloads and feature flags have the same semantics as
/// [`encode_lossless_rgba_with_metadata`]. The default options are
/// byte-for-byte equivalent to that existing entry point. Profile selection
/// never drops or rewrites ICCP, EXIF, or XMP payload bytes.
///
/// # Errors
///
/// Returns the same errors as [`encode_lossless_rgba_with_metadata`].
pub fn encode_lossless_rgba_with_metadata_and_options(
    width: u32,
    height: u32,
    rgba: &[u8],
    metadata: &Metadata,
    options: LosslessEncodeOptions,
) -> Result<Vec<u8>, EncodeError> {
    if options.profile == LosslessEncodeProfile::Default {
        return encode_lossless_rgba_with_metadata(width, height, rgba, metadata);
    }
    let has_alpha = rgba.chunks_exact(4).any(|pixel| pixel[3] != u8::MAX);
    let riff = encode_lossless_rgba_with_options(width, height, rgba, options)?;
    let payload = copy_vp8l_payload(&riff)?;
    wrap_vp8l_with_metadata(payload, width, height, has_alpha, metadata)
}

fn wrap_vp8l(payload: Vec<u8>) -> Result<Vec<u8>, EncodeError> {
    webp_container::serialize_vp8l(payload, 0, 0, false, webp_container::Metadata::default())
        .map_err(map_container_error)
}

pub(crate) fn copy_vp8l_payload(riff: &[u8]) -> Result<Vec<u8>, EncodeError> {
    let parsed = webp_container::parse(
        riff,
        webp_container::CompatibilityProfile::SpecStrict,
        &webp_container::ContainerLimits::default(),
    )
    .map_err(|_| EncodeError::output_size_overflow())?;
    let payload = parsed
        .chunks()
        .iter()
        .find(|chunk| chunk.fourcc == webp_container::VP8L)
        .map(|chunk| chunk.payload)
        .ok_or_else(EncodeError::output_size_overflow)?;
    let mut copy = Vec::new();
    copy.try_reserve_exact(payload.len())
        .map_err(|_| EncodeError::allocation_failed())?;
    copy.extend_from_slice(payload);
    Ok(copy)
}

fn wrap_vp8(
    payload: Vec<u8>,
    width: u32,
    height: u32,
    alpha: Option<Vec<u8>>,
    alpha_options: crate::alpha::AlphaEncodeOptions,
) -> Result<Vec<u8>, EncodeError> {
    let alpha_payload = alpha
        .map(|samples| {
            crate::alpha::encode(&samples, width, height, alpha_options)
                .map_err(map_alpha_encode_error)
        })
        .transpose()?;
    webp_container::serialize_vp8(payload, width, height, alpha_payload.as_deref())
        .map_err(map_container_error)
}

fn map_vp8_encode_error(error: crate::vp8::Vp8EncodeError) -> EncodeError {
    match error {
        crate::vp8::Vp8EncodeError::InvalidDimensions => EncodeError::invalid_dimensions(),
        crate::vp8::Vp8EncodeError::InvalidRgbaLength => EncodeError::invalid_rgba_length(),
        crate::vp8::Vp8EncodeError::AllocationFailed => EncodeError::allocation_failed(),
        crate::vp8::Vp8EncodeError::FirstPartitionTooLarge
        | crate::vp8::Vp8EncodeError::InvalidPlaneLayout
        | crate::vp8::Vp8EncodeError::InvalidQuantizer => EncodeError::unsupported_lossy_profile(),
    }
}

fn map_alpha_encode_error(error: crate::alpha::AlphaEncodeError) -> EncodeError {
    match error {
        crate::alpha::AlphaEncodeError::InvalidDimensions => EncodeError::invalid_dimensions(),
        crate::alpha::AlphaEncodeError::InvalidSampleLength => EncodeError::invalid_rgba_length(),
        crate::alpha::AlphaEncodeError::SizeOverflow => EncodeError::output_size_overflow(),
        crate::alpha::AlphaEncodeError::AllocationFailed => EncodeError::allocation_failed(),
        crate::alpha::AlphaEncodeError::InvalidQuality => EncodeError::invalid_quality(),
    }
}

fn map_container_error(error: webp_container::ContainerError) -> EncodeError {
    match error.kind() {
        webp_container::ContainerErrorKind::SizeOverflow => EncodeError::output_size_overflow(),
        webp_container::ContainerErrorKind::AllocationFailed => EncodeError::allocation_failed(),
        webp_container::ContainerErrorKind::InvalidDimensions => EncodeError::invalid_dimensions(),
        webp_container::ContainerErrorKind::InvalidAnimation => EncodeError::invalid_animation(),
        webp_container::ContainerErrorKind::UnexpectedEof
        | webp_container::ContainerErrorKind::InvalidContainer
        | webp_container::ContainerErrorKind::LimitExceeded => EncodeError::output_size_overflow(),
    }
}

pub(crate) fn wrap_vp8l_with_metadata(
    payload: Vec<u8>,
    width: u32,
    height: u32,
    has_alpha: bool,
    metadata: &Metadata,
) -> Result<Vec<u8>, EncodeError> {
    webp_container::serialize_vp8l(
        payload,
        width,
        height,
        has_alpha,
        borrowed_metadata(metadata),
    )
    .map_err(map_container_error)
}

fn borrowed_metadata(metadata: &Metadata) -> webp_container::Metadata<'_> {
    webp_container::Metadata {
        iccp: metadata.iccp.as_deref(),
        exif: metadata.exif.as_deref(),
        xmp: metadata.xmp.as_deref(),
    }
}

#[cfg(test)]
#[path = "writer_tests.rs"]
mod encoder_tests;
