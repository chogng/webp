#![forbid(unsafe_code)]
//! Complete WebP `ALPH` payload encoding and decoding.

mod alpha;
mod encode;
mod encode_filter;
mod encode_huffman;
mod encode_lz77;
mod encode_palette;
mod encode_token_output;
mod level_reduction;

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
pub use encode_filter::AlphaFilterSelection;
#[cfg(feature = "benchmark-internals")]
#[doc(hidden)]
pub use encode_token_output::BenchmarkWriterVariant;
#[cfg(feature = "benchmark-internals")]
#[doc(hidden)]
pub use encode_token_output::set_benchmark_writer_variant;
