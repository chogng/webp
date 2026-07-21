#![forbid(unsafe_code)]
//! Stable public API for the safe WebP implementation.
//!
//! M1 decodes static VP8L images to canonical RGBA8. M2 decodes VP8 key
//! frames to canonical RGBA8. M3 adds `ALPH` planes and animation decoding.
//! M4 begins static lossless VP8L encoding.

pub use webp_core::{CompatibilityProfile, DecodeError, DecodeErrorKind, DecodeLimits};

mod api;
mod decoder;
mod encoder;
mod incremental;
mod info;

pub use api::{
    Animation, AnimationEncodeFrame, AnimationEncodeOptions, AnimationFrame, DecodeOptions,
    EncodeError, Image, ImageInfo, Metadata, Progress,
};
pub use incremental::IncrementalDecoder;
pub use info::{read_info, read_metadata};

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

/// Encodes a static straight-RGBA8 image as a lossless WebP file.
///
/// M4+ writes VP8L residuals with bounded cache selection, small-palette
/// indexing, and deterministic Huffman coding. The output is valid and
/// lossless, without a compression-ratio or throughput guarantee.
pub use encoder::{
    encode_lossless_animation, encode_lossless_animation_with_metadata, encode_lossless_rgba,
    encode_lossless_rgba_with_metadata,
};
