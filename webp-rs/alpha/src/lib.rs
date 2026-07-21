#![forbid(unsafe_code)]
//! Complete WebP `ALPH` payload encoding and decoding.

mod alpha;
mod encode;

pub use alpha::AlphaCompression;
pub use alpha::AlphaFilter;
pub use alpha::AlphaHeader;
pub use alpha::AlphaPreprocessing;
pub use alpha::decode;
pub use alpha::decode_raw;
pub use alpha::parse_header;
pub use encode::AlphaEncodeError;
pub use encode::AlphaEncodeOptions;
pub use encode::encode;
