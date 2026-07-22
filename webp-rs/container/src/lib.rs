#![forbid(unsafe_code)]
//! Public surface for safe, zero-copy WebP RIFF container parsing.

mod animation;
mod arithmetic;
mod chunk;
mod demux;
mod editor;
mod error;
mod fourcc;
mod layout;
mod metadata;
mod mux;
mod options;

pub use animation::Animation;
pub use animation::AnimationFrame;
pub use animation::FrameBitstream;
pub use chunk::Chunk;
pub use chunk::MuxChunk;
pub use demux::Container;
pub use demux::Demuxer;
pub use demux::ImageBitstream;
pub use demux::StillImage;
pub use demux::parse;
pub use editor::Editor;
pub use error::ContainerError;
pub use error::ContainerErrorKind;
pub use fourcc::ALPH;
pub use fourcc::ANIM;
pub use fourcc::ANMF;
pub use fourcc::EXIF;
pub use fourcc::FourCc;
pub use fourcc::ICCP;
pub use fourcc::VP8;
pub use fourcc::VP8L;
pub use fourcc::VP8X;
pub use fourcc::XMP;
pub use layout::Vp8x;
pub use layout::Vp8xFlags;
pub use metadata::Metadata;
pub use mux::AnimationFrameInput;
#[doc(hidden)]
pub use mux::AnimationFrameMux;
#[doc(hidden)]
pub use mux::AnimationMuxOptions;
pub use mux::FramePayload;
pub use mux::Muxer;
#[doc(hidden)]
pub use mux::serialize_animation;
#[doc(hidden)]
pub use mux::serialize_animation_frame;
#[doc(hidden)]
pub use mux::serialize_vp8;
#[doc(hidden)]
pub use mux::serialize_vp8l;
pub use options::CompatibilityProfile;
pub use options::ContainerLimits;
pub use options::DemuxOptions;
