#![forbid(unsafe_code)]
//! WebP `ALPH` payload support, separated by operation and wire ownership.
//!
//! [`decode`] owns recovery of a stored alpha payload. [`encode`] owns
//! construction and compression choices for a new payload. [`wire`] owns the
//! `ALPH` header fields shared by both directions.

pub(crate) mod decode;
mod wire;

pub use decode::decode;
pub use decode::decode_raw;
pub use decode::parse_header;
pub use wire::AlphaCompression;
pub use wire::AlphaFilter;
pub use wire::AlphaHeader;
pub use wire::AlphaPreprocessing;
