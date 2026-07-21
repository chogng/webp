#![forbid(unsafe_code)]
//! Public surface for WebP alpha-plane decoding.

mod alpha;

pub use alpha::AlphaCompression;
pub use alpha::AlphaFilter;
pub use alpha::AlphaHeader;
pub use alpha::decode;
pub use alpha::decode_raw;
pub use alpha::parse_header;
