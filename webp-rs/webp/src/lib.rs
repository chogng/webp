#![forbid(unsafe_code)]
//! Stable public API for the safe WebP implementation.
//!
//! M1 decodes static VP8L images to canonical RGBA8. M2 decodes VP8 key
//! frames to canonical RGBA8. M3 adds `ALPH` planes and animation decoding.
//! M4 begins static lossless VP8L encoding.

pub use alpha::AlphaCompression;
pub use alpha::AlphaEncodeOptions;
pub use alpha::AlphaFilter;
pub use alpha::AlphaFilterSelection;
pub use error::DecodeError;
pub use error::DecodeErrorKind;
pub use error::EncodeError;
pub use limits::CompatibilityProfile;
pub use limits::DecodeLimits;

#[allow(dead_code, unused_imports)] // Private owner keeps reference/test entry points.
mod alpha;
mod animated_image;
#[allow(dead_code)] // Canvas geometry accessors are retained for sibling tests.
mod animation;
mod api;
#[allow(dead_code)] // Buffered lookahead helpers remain fuzzed through codec readers.
mod bit_io;
mod container_adapter;
mod error;
#[cfg(feature = "fuzzing")]
#[doc(hidden)]
pub mod fuzzing;
mod incremental;
mod inspection;
mod limits;
mod static_image;
#[allow(dead_code, unused_imports)] // Private owner keeps reference/test entry points.
mod vp8;
mod vp8l;

pub(crate) use bit_io::BitReader;
pub(crate) use bit_io::BitWriter;
pub(crate) use bit_io::ShiftedBitReader;
pub(crate) use limits::WorkBudget;
pub(crate) use limits::checked_chunk_end;
pub(crate) use limits::checked_image_bytes;
pub(crate) use limits::checked_rect_end;

pub use api::Animation;
pub use api::AnimationEncodeFrame;
pub use api::AnimationEncodeOptions;
pub use api::AnimationFrame;
pub use api::DecodeOptions;
pub use api::Image;
pub use api::ImageInfo;
pub use api::LosslessEncodeOptions;
pub use api::LosslessEncodeProfile;
pub use api::LossyEncodeOptions;
pub use api::Metadata;
pub use api::Progress;
pub use incremental::IncrementalDecoder;
pub use inspection::read_info;
pub use inspection::read_metadata;

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
    static_image::decode(data, options)
}

/// Decodes an animated WebP into display-ready straight-RGBA8 canvas frames.
///
/// Each returned frame contains the full canvas after blending and disposal.
/// Static images are rejected.
pub fn decode_animation(data: &[u8], options: &DecodeOptions) -> Result<Animation, DecodeError> {
    animated_image::decode_animation(data, options)
}

pub use animated_image::encode_lossless_animation;
pub use animated_image::encode_lossless_animation_with_metadata;
pub use static_image::encode_lossless_rgba;
pub use static_image::encode_lossless_rgba_with_metadata;
pub use static_image::encode_lossless_rgba_with_metadata_and_options;
pub use static_image::encode_lossless_rgba_with_options;
pub use static_image::encode_lossy_rgba;
pub use static_image::encode_lossy_rgba_with_alpha_options;
pub use static_image::encode_lossy_rgba_with_options;
