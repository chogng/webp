#![forbid(unsafe_code)]
//! Safe, zero-copy parsing of complete WebP RIFF containers.
//!
//! The crate owns RIFF/chunk layout, fixed VP8/VP8L image-header inspection,
//! borrowed frame and metadata views, compatibility policy, and resource
//! limits. Incremental input state belongs to `webp-decode`; pixel decoding
//! remains with the codec crates.

mod animation;
mod arithmetic;
mod chunk;
mod demux;
mod image_header;
mod layout;
mod options;

pub use animation::Animation;
pub use animation::AnimationFrame;
pub use animation::FrameBitstream;
pub use chunk::Chunk;
pub use demux::Container;
pub use demux::Demuxer;
pub use demux::ImageBitstream;
pub use demux::StillImage;
pub use demux::parse;
pub use options::CompatibilityProfile;
pub use options::ContainerLimits;
pub use options::DemuxOptions;
pub use webp_container::ALPH;
pub use webp_container::ANIM;
pub use webp_container::ANMF;
pub use webp_container::ContainerError;
pub use webp_container::ContainerErrorKind;
pub use webp_container::EXIF;
pub use webp_container::FourCc;
pub use webp_container::ICCP;
pub use webp_container::Metadata;
pub use webp_container::VP8;
pub use webp_container::VP8L;
pub use webp_container::VP8X;
pub use webp_container::Vp8x;
pub use webp_container::Vp8xFlags;
pub use webp_container::XMP;
