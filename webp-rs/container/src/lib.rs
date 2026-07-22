#![forbid(unsafe_code)]
//! Shared WebP RIFF container vocabulary and wire models.

mod error;
mod fourcc;
mod metadata;
mod vp8x;

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
#[doc(hidden)]
pub use fourcc::is_known;
pub use metadata::Metadata;
pub use vp8x::Vp8x;
pub use vp8x::Vp8xFlags;
