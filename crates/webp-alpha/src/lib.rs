#![forbid(unsafe_code)]
//! Public surface for WebP alpha-plane decoding.

mod alpha;

pub use alpha::{AlphaCompression, AlphaFilter, AlphaHeader, decode, decode_raw, parse_header};
