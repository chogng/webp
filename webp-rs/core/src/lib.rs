#![forbid(unsafe_code)]
//! Shared, allocation-conscious primitives used by the WebP parser and codecs.

mod bit_io;
mod error;
mod limits;

pub use bit_io::BitReader;
pub use bit_io::BitWriter;
pub use bit_io::ShiftedBitReader;
pub use error::DecodeError;
pub use error::DecodeErrorKind;
pub use limits::CompatibilityProfile;
pub use limits::DecodeLimits;
pub use limits::WorkBudget;
pub use limits::checked_chunk_end;
pub use limits::checked_image_bytes;
pub use limits::checked_rect_end;
