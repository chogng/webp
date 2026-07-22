#![forbid(unsafe_code)]
//! Direction-specific public API for encoding WebP images.

mod alpha;
#[cfg(feature = "animation")]
mod animated_image;
#[cfg(feature = "animation")]
mod animation;
mod error;
mod options;
mod static_image;
mod vp8;
mod vp8l;

pub use alpha::AlphaEncodeError;
pub use alpha::AlphaEncodeOptions;
pub use alpha::AlphaFilterSelection;
#[cfg(feature = "alpha-benchmark-internals")]
#[doc(hidden)]
pub use alpha::BenchmarkWriterVariant;
pub use alpha::encode as encode_alpha;
#[cfg(feature = "alpha-benchmark-internals")]
#[doc(hidden)]
pub use alpha::set_benchmark_writer_variant;
pub use error::EncodeError;
pub use options::LosslessEncodeOptions;
pub use options::LosslessEncodeProfile;
pub use options::LossyEncodeOptions;
pub use static_image::encode_lossless_rgba;
pub use static_image::encode_lossless_rgba_with_metadata;
pub use static_image::encode_lossless_rgba_with_metadata_and_options;
pub use static_image::encode_lossless_rgba_with_options;
pub use static_image::encode_lossy_rgba;
pub use static_image::encode_lossy_rgba_with_alpha_options;
pub use static_image::encode_lossy_rgba_with_options;
pub use webp_container::AlphaCompression;
pub use webp_container::AlphaFilter;
pub use webp_decode::Metadata;

#[cfg(feature = "animation")]
pub use animated_image::encode_lossless_animation;
#[cfg(feature = "animation")]
pub use animated_image::encode_lossless_animation_with_metadata;
#[cfg(feature = "animation")]
pub use animation::AnimationEncodeFrame;
#[cfg(feature = "animation")]
pub use animation::AnimationEncodeOptions;
