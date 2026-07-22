//! Stable public data models shared by WebP operations.

mod animation;
mod image;
mod metadata;
mod options;

#[cfg(feature = "animation")]
pub use animation::Animation;
#[cfg(feature = "animation")]
pub use animation::AnimationFrame;
#[cfg(feature = "decode")]
pub use image::Image;
#[cfg(feature = "decode")]
pub use image::ImageInfo;
#[cfg(feature = "decode")]
pub use image::IncrementalImage;
#[cfg(feature = "decode")]
pub use image::Progress;
#[cfg(any(feature = "decode", feature = "encode"))]
pub use metadata::Metadata;
#[cfg(feature = "decode")]
pub use options::DecodeOptions;
