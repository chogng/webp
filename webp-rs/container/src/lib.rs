#![forbid(unsafe_code)]
//! Public surface for safe, zero-copy WebP RIFF container parsing.

mod container;

pub use container::ALPH;
pub use container::ANIM;
pub use container::ANMF;
pub use container::Animation;
pub use container::AnimationFrame;
pub use container::Chunk;
pub use container::Container;
pub use container::EXIF;
pub use container::FourCc;
pub use container::FrameBitstream;
pub use container::ICCP;
pub use container::Metadata;
pub use container::VP8;
pub use container::VP8L;
pub use container::VP8X;
pub use container::Vp8x;
pub use container::Vp8xFlags;
pub use container::XMP;
pub use container::parse;
