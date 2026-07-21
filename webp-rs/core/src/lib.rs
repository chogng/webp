#![forbid(unsafe_code)]
//! Shared, allocation-conscious primitives used by the WebP parser and codecs.

mod bit_io;
mod error;
mod limits;

pub use bit_io::{BitReader, BitWriter, ShiftedBitReader};
pub use error::{DecodeError, DecodeErrorKind};
pub use limits::{
    CompatibilityProfile, DecodeLimits, WorkBudget, checked_chunk_end, checked_image_bytes,
    checked_rect_end,
};
