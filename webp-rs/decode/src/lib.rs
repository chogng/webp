#![forbid(unsafe_code)]
//! Stable public API for the safe WebP implementation.
//!
//! M1 decodes static VP8L images to canonical RGBA8. M2 decodes VP8 key
//! frames to canonical RGBA8. M3 adds `ALPH` planes and animation decoding.
//! M4 begins static lossless VP8L encoding. The default feature set preserves
//! this complete API; consumers can select `decode` alone to exclude encoder
//! orchestration, or add `animation` and `encode` independently.

pub use error::DecodeError;
pub use error::DecodeErrorKind;
#[cfg(feature = "decode")]
pub use limits::CompatibilityProfile;
#[cfg(feature = "decode")]
pub use limits::DecodeLimits;

#[cfg(feature = "decode")]
#[allow(dead_code, unused_imports)] // Private owner keeps reference/test entry points.
mod alpha;
#[cfg(feature = "animation")]
mod animated_image;
#[cfg(feature = "animation")]
#[allow(dead_code)] // Canvas geometry accessors are retained for sibling tests.
mod animation;
mod api;
#[cfg(feature = "decode")]
#[allow(dead_code)] // Buffered lookahead helpers remain fuzzed through codec readers.
mod bit_io;
#[cfg(feature = "decode")]
mod container_adapter;
mod error;
#[cfg(feature = "fuzzing")]
#[doc(hidden)]
pub mod fuzzing;
#[cfg(feature = "decode")]
mod incremental;
#[cfg(feature = "decode")]
mod inspection;
#[cfg(feature = "decode")]
mod limits;
#[cfg(feature = "decode")]
mod static_image;
#[cfg(feature = "decode")]
#[allow(dead_code, unused_imports)] // Private owner keeps reference/test entry points.
mod vp8;
#[cfg(feature = "decode")]
mod vp8l;

#[cfg(feature = "decode")]
pub(crate) use bit_io::BitReader;
#[cfg(feature = "decode")]
pub(crate) use bit_io::BitWriter;
#[cfg(feature = "decode")]
pub(crate) use bit_io::ShiftedBitReader;
#[cfg(feature = "decode")]
pub(crate) use limits::WorkBudget;
#[cfg(feature = "decode")]
pub(crate) use limits::checked_chunk_end;
#[cfg(feature = "decode")]
pub(crate) use limits::checked_image_bytes;
#[cfg(all(feature = "decode", feature = "animation"))]
pub(crate) use limits::checked_rect_end;

#[cfg(feature = "animation")]
pub use api::Animation;
#[cfg(feature = "animation")]
pub use api::AnimationFrame;
#[cfg(feature = "decode")]
pub use api::DecodeOptions;
#[cfg(feature = "decode")]
pub use api::Image;
#[cfg(feature = "decode")]
pub use api::ImageInfo;
#[cfg(feature = "decode")]
pub use api::IncrementalImage;
#[cfg(any(feature = "decode", feature = "encode"))]
pub use api::Metadata;
#[cfg(feature = "decode")]
pub use api::Progress;
#[cfg(feature = "decode")]
pub use incremental::IncrementalDecoder;
#[cfg(feature = "decode")]
pub use inspection::read_info;
#[cfg(feature = "decode")]
pub use inspection::read_metadata;

/// Decodes a supported static WebP image to straight RGBA8.
///
/// M1 supports static VP8L images, including transforms, color cache,
/// meta-Huffman groups, and backward references. M2 supports VP8 key frames.
/// M3 supports their `ALPH` planes. With the `animation` feature, animated
/// containers use the separate animation decode API; incremental codec state
/// remains unavailable.
///
/// # Errors
///
/// Returns container-validation, codec, resource-limit, or unsupported-feature
/// errors. The function never substitutes an incomplete decode result.
#[cfg(feature = "decode")]
pub fn decode(data: &[u8], options: &DecodeOptions) -> Result<Image, DecodeError> {
    static_image::decode(data, options)
}

/// Decodes an animated WebP into display-ready straight-RGBA8 canvas frames.
///
/// Each returned frame contains the full canvas after blending and disposal.
/// Static images are rejected.
#[cfg(all(feature = "decode", feature = "animation"))]
pub fn decode_animation(data: &[u8], options: &DecodeOptions) -> Result<Animation, DecodeError> {
    animated_image::decode_animation(data, options)
}

/// Unstable VP8 primitives temporarily shared by the reader and writer.
///
/// Frame writing and RGB-to-YUV conversion are owned by `webp-encode`. This
/// module contains only codec algorithms that still need a dedicated shared
/// home while the VP8 reader is separated further. It is not a compatibility
/// API and may change without notice.
#[cfg(feature = "encode")]
#[doc(hidden)]
pub mod vp8_codec {
    pub use crate::vp8::BoolEncodeError;
    pub use crate::vp8::BoolEncoder;
    pub use crate::vp8::COEFFICIENT_BANDS;
    pub use crate::vp8::COEFFICIENT_DEFAULTS;
    pub use crate::vp8::COEFFICIENT_UPDATE_PROBABILITIES;
    pub use crate::vp8::ChromaMode;
    pub use crate::vp8::CoefficientBlockType;
    pub use crate::vp8::CoefficientEncodeError;
    pub use crate::vp8::CoefficientProbabilities;
    pub use crate::vp8::DecodedCoefficients;
    pub use crate::vp8::DequantizationMatrix;
    pub use crate::vp8::Intra16Mode;
    pub use crate::vp8::IntraMacroblock;
    pub use crate::vp8::LumaMode;
    pub use crate::vp8::MacroblockPixels;
    pub use crate::vp8::MacroblockPredictionEdges;
    pub use crate::vp8::MacroblockResiduals;
    pub use crate::vp8::QuantizationHeader;
    pub use crate::vp8::SegmentHeader;
    pub use crate::vp8::decode_intra_frame;
    pub use crate::vp8::derive_dequantization;
    pub use crate::vp8::encode_coefficients;
    pub use crate::vp8::encode_coefficients_observed;
    pub use crate::vp8::parse_partition_layout;
    pub use crate::vp8::parse_riff_payload;
    pub use crate::vp8::predict_intra16_macroblock;
    pub use crate::vp8::quantize_block;
    pub use crate::vp8::reconstruct_intra_macroblock;
    pub use crate::vp8::reconstruct_intra16_chroma;
    pub use crate::vp8::reconstruct_intra16_luma;
}
