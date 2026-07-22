#![forbid(unsafe_code)]
//! Complete WebP `ALPH` payload encoding and decoding.

mod backward_references;
mod encode_token_output;
mod filters;
mod level_reduction;
mod palette_plan;
mod plane_reader;
mod plane_writer;
mod symbol_plan;

pub use filters::AlphaFilterSelection;
pub use plane_reader::AlphaCompression;
pub use plane_reader::AlphaFilter;
pub use plane_reader::AlphaHeader;
pub use plane_reader::AlphaPreprocessing;
pub use plane_reader::decode;
pub use plane_reader::decode_raw;
pub use plane_reader::parse_header;
pub use plane_writer::AlphaEncodeError;
pub use plane_writer::AlphaEncodeOptions;
pub use plane_writer::encode;

#[cfg(feature = "alpha-benchmark-internals")]
pub use encode_token_output::BenchmarkWriterVariant;
#[cfg(feature = "alpha-benchmark-internals")]
pub use encode_token_output::set_benchmark_writer_variant;
