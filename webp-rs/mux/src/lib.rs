#![forbid(unsafe_code)]
//! Safe WebP RIFF construction and lossless container editing.

mod chunk;
mod editor;
mod mux;

pub use chunk::MuxChunk;
pub use editor::Editor;
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
pub use webp_container::XMP;
pub use webp_demux::CompatibilityProfile;
pub use webp_demux::ContainerLimits;
pub use webp_demux::DemuxOptions;
pub use webp_demux::Demuxer;
pub use webp_demux::parse;
