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
#[cfg(feature = "encode")]
pub use error::EncodeError;
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

/// Unstable implementation hooks consumed by the direction-specific encoder.
///
/// This module is not part of the compatibility API. It exists while shared
/// codec primitives are split from their reader and writer orchestration.
#[cfg(feature = "encode")]
#[doc(hidden)]
pub mod encode_support {
    pub use crate::alpha::AlphaCompression;
    pub use crate::alpha::AlphaFilter;
    pub use crate::alpha::AlphaHeader;
    pub use crate::alpha::AlphaPreprocessing;
    pub use crate::bit_io::BitWriter;
    pub use crate::error::EncodeError;
    pub use crate::vp8::Vp8EncodeError;
    pub use crate::vp8::encode_dc_predicted_key_frame_with_quantizer;
    pub use crate::vp8::rgba_to_yuv420;
    pub use crate::vp8l::header::MAX_DIMENSION as MAX_VP8L_DIMENSION;
    pub use crate::vp8l::huffman::symbol_writer::EncodingTable;
    pub use crate::vp8l::huffman::symbol_writer::WireWriteError;
    pub use crate::vp8l::huffman::symbol_writer::canonical_table;
    pub use crate::vp8l::huffman::symbol_writer::table_from_codes_for_test;
    pub use crate::vp8l::huffman::symbol_writer::table_wire_symbol;
    pub use crate::vp8l::huffman::symbol_writer::write_simple_table;
    pub use crate::vp8l::huffman::symbol_writer::write_table_symbol;
    pub use crate::vp8l::image_writer::COLOR_TRANSFORM_BLOCK_BITS;
    pub use crate::vp8l::image_writer::ColorTransformPlan;
    pub use crate::vp8l::image_writer::EntropyToken;
    pub use crate::vp8l::image_writer::collect_entropy_tokens;
    pub use crate::vp8l::image_writer::select_color_cache_bits;
    pub use crate::vp8l::image_writer::select_color_transform;
    pub use crate::vp8l::image_writer::select_left_predictor;
    pub use crate::vp8l::image_writer::try_make_palette_plan;

    /// Internal VP8L spatial writer selection.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum Vp8lSpatialProfile {
        Compact,
        LowLatency,
    }

    pub fn encode_vp8l_payload(
        width: u32,
        height: u32,
        rgba: &[u8],
    ) -> Result<(Vec<u8>, bool), EncodeError> {
        crate::vp8l::image_writer::encode_vp8l_payload(width, height, rgba)
    }

    pub fn encode_vp8l_spatial(
        width: u32,
        height: u32,
        rgba: &[u8],
        profile: Vp8lSpatialProfile,
    ) -> Result<Vec<u8>, EncodeError> {
        let profile = match profile {
            Vp8lSpatialProfile::Compact => {
                crate::vp8l::image_writer::spatial_plan::SpatialProfile::Compact
            }
            Vp8lSpatialProfile::LowLatency => {
                crate::vp8l::image_writer::spatial_plan::SpatialProfile::LowLatency
            }
        };
        crate::vp8l::image_writer::spatial_writer::encode_profile(width, height, rgba, profile)
    }

    pub fn validate_vp8l_input(width: u32, height: u32, rgba: &[u8]) -> Result<(), EncodeError> {
        crate::vp8l::image_writer::validate_input(width, height, rgba)
    }
}
