#![forbid(unsafe_code)]
//! WebP `ALPH` payload support, separated by operation and wire ownership.
//!
//! [`decode`] owns recovery of a stored alpha payload. [`encode`] owns
//! construction and compression choices for a new payload. [`wire`] owns the
//! `ALPH` header fields shared by both directions.

pub(crate) mod decode;

pub use decode::decode;
pub use decode::decode_raw;
pub use decode::parse_header;
pub use webp_container::AlphaCompression;
pub use webp_container::AlphaFilter;
pub use webp_container::AlphaHeader;
pub use webp_container::AlphaPreprocessing;
