#![forbid(unsafe_code)]
//! Stable public API for the safe WebP implementation.
//!
//! M1 decodes static VP8L images to canonical RGBA8. M2 decodes VP8 key
//! frames to canonical RGBA8. M3 adds `ALPH` planes and animation decoding.
//! M4 begins static lossless VP8L encoding.

pub use webp_alpha::AlphaCompression;
pub use webp_alpha::AlphaEncodeOptions;
pub use webp_alpha::AlphaFilter;
pub use webp_alpha::AlphaFilterSelection;
pub use webp_core::CompatibilityProfile;
pub use webp_core::DecodeError;
pub use webp_core::DecodeErrorKind;
pub use webp_core::DecodeLimits;

mod api;
mod decoder;
mod encoder;
mod incremental;
mod info;
mod vp8l;

pub use api::Animation;
pub use api::AnimationEncodeFrame;
pub use api::AnimationEncodeOptions;
pub use api::AnimationFrame;
pub use api::DecodeOptions;
pub use api::EncodeError;
pub use api::Image;
pub use api::ImageInfo;
pub use api::LosslessEncodeOptions;
pub use api::LosslessEncodeProfile;
pub use api::LossyEncodeOptions;
pub use api::Metadata;
pub use api::Progress;
pub use incremental::IncrementalDecoder;
pub use info::read_info;
pub use info::read_metadata;

/// Decodes a supported static WebP image to straight RGBA8.
///
/// M1 supports static VP8L images, including transforms, color cache,
/// meta-Huffman groups, and backward references. M2 supports VP8 key frames.
/// M3 supports their `ALPH` planes. Use [`decode_animation`] for animated
/// containers; incremental codec state remains unavailable.
///
/// # Errors
///
/// Returns container-validation, codec, resource-limit, or unsupported-feature
/// errors. The function never substitutes an incomplete decode result.
pub fn decode(data: &[u8], options: &DecodeOptions) -> Result<Image, DecodeError> {
    decoder::decode(data, options)
}

/// Decodes an animated WebP into display-ready straight-RGBA8 canvas frames.
///
/// Each returned frame contains the full canvas after blending and disposal.
/// Static images are rejected.
pub fn decode_animation(data: &[u8], options: &DecodeOptions) -> Result<Animation, DecodeError> {
    decoder::decode_animation(data, options)
}

pub use encoder::encode_lossless_animation;
pub use encoder::encode_lossless_animation_with_metadata;
pub use encoder::encode_lossless_rgba;
pub use encoder::encode_lossless_rgba_with_metadata;
pub use encoder::encode_lossless_rgba_with_metadata_and_options;
pub use encoder::encode_lossless_rgba_with_options;
pub use encoder::encode_lossy_rgba;
pub use encoder::encode_lossy_rgba_with_alpha_options;
pub use encoder::encode_lossy_rgba_with_options;
