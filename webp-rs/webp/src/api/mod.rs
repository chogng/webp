//! Stable public data models shared by WebP operations.

mod animation;
mod image;
mod metadata;
mod options;

pub use animation::Animation;
pub use animation::AnimationEncodeFrame;
pub use animation::AnimationEncodeOptions;
pub use animation::AnimationFrame;
pub use image::Image;
pub use image::ImageInfo;
pub use image::Progress;
pub use metadata::Metadata;
pub use options::DecodeOptions;
pub use options::LosslessEncodeOptions;
pub use options::LosslessEncodeProfile;
pub use options::LossyEncodeOptions;
