#![forbid(unsafe_code)]
//! Unified compatibility facade for the safe WebP workspace.

#[cfg(feature = "decode")]
pub use webp_decode::CompatibilityProfile;
#[cfg(feature = "decode")]
pub use webp_decode::DecodeError;
#[cfg(feature = "decode")]
pub use webp_decode::DecodeErrorKind;
#[cfg(feature = "decode")]
pub use webp_decode::DecodeLimits;
#[cfg(feature = "decode")]
pub use webp_decode::DecodeOptions;
#[cfg(feature = "decode")]
pub use webp_decode::Image;
#[cfg(feature = "decode")]
pub use webp_decode::ImageInfo;
#[cfg(feature = "decode")]
pub use webp_decode::IncrementalDecoder;
#[cfg(feature = "decode")]
pub use webp_decode::IncrementalImage;
#[cfg(feature = "decode")]
pub use webp_decode::Progress;
#[cfg(feature = "decode")]
pub use webp_decode::decode;
#[cfg(feature = "decode")]
pub use webp_decode::read_info;
#[cfg(feature = "decode")]
pub use webp_decode::read_metadata;

#[cfg(all(feature = "decode", feature = "animation"))]
pub use webp_decode::Animation;
#[cfg(all(feature = "decode", feature = "animation"))]
pub use webp_decode::AnimationFrame;
#[cfg(all(feature = "decode", feature = "animation"))]
pub use webp_decode::decode_animation;

#[cfg(feature = "encode")]
pub use webp_encode::AlphaCompression;
#[cfg(feature = "encode")]
pub use webp_encode::AlphaEncodeOptions;
#[cfg(feature = "encode")]
pub use webp_encode::AlphaFilter;
#[cfg(feature = "encode")]
pub use webp_encode::AlphaFilterSelection;
#[cfg(feature = "encode")]
pub use webp_encode::EncodeError;
#[cfg(feature = "encode")]
pub use webp_encode::LosslessEncodeOptions;
#[cfg(feature = "encode")]
pub use webp_encode::LosslessEncodeProfile;
#[cfg(feature = "encode")]
pub use webp_encode::LossyEncodeOptions;
#[cfg(feature = "encode")]
pub use webp_encode::encode_lossless_rgba;
#[cfg(feature = "encode")]
pub use webp_encode::encode_lossless_rgba_with_metadata;
#[cfg(feature = "encode")]
pub use webp_encode::encode_lossless_rgba_with_metadata_and_options;
#[cfg(feature = "encode")]
pub use webp_encode::encode_lossless_rgba_with_options;
#[cfg(feature = "encode")]
pub use webp_encode::encode_lossy_rgba;
#[cfg(feature = "encode")]
pub use webp_encode::encode_lossy_rgba_with_alpha_options;
#[cfg(feature = "encode")]
pub use webp_encode::encode_lossy_rgba_with_options;

#[cfg(all(feature = "encode", feature = "animation"))]
pub use webp_encode::AnimationEncodeFrame;
#[cfg(all(feature = "encode", feature = "animation"))]
pub use webp_encode::AnimationEncodeOptions;
#[cfg(all(feature = "encode", feature = "animation"))]
pub use webp_encode::encode_lossless_animation;
#[cfg(all(feature = "encode", feature = "animation"))]
pub use webp_encode::encode_lossless_animation_with_metadata;

#[cfg(feature = "decode")]
pub use webp_decode::Metadata;
#[cfg(all(feature = "encode", not(feature = "decode")))]
pub use webp_encode::Metadata;

#[cfg(feature = "fuzzing")]
#[doc(hidden)]
pub mod fuzzing {
    pub use webp_decode::fuzzing::{vp8_bool, vp8_coefficients, vp8_partition, vp8_residuals};
    pub use webp_decode::fuzzing::{vp8_transforms, vp8l_huffman, vp8l_transforms};
    #[cfg(feature = "alpha-benchmark-internals")]
    pub use webp_encode::BenchmarkWriterVariant;
    pub use webp_encode::encode_alpha;
    #[cfg(feature = "alpha-benchmark-internals")]
    pub use webp_encode::set_benchmark_writer_variant;
}
