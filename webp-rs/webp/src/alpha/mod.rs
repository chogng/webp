#![forbid(unsafe_code)]
//! Complete WebP `ALPH` payload encoding and decoding.

#[cfg(feature = "encode")]
mod backward_references;
#[cfg(feature = "encode")]
mod encode_token_output;
#[cfg(feature = "encode")]
mod filters;
#[cfg(feature = "encode")]
mod level_reduction;
#[cfg(feature = "encode")]
mod palette_plan;
mod plane_reader;
#[cfg(feature = "encode")]
mod plane_writer;
#[cfg(feature = "encode")]
mod symbol_plan;

#[cfg(feature = "encode")]
pub use filters::AlphaFilterSelection;
pub use plane_reader::AlphaCompression;
pub use plane_reader::AlphaFilter;
pub use plane_reader::AlphaHeader;
pub use plane_reader::AlphaPreprocessing;
pub use plane_reader::decode;
pub use plane_reader::decode_raw;
pub use plane_reader::parse_header;
#[cfg(feature = "encode")]
pub use plane_writer::AlphaEncodeError;
#[cfg(feature = "encode")]
pub use plane_writer::AlphaEncodeOptions;
#[cfg(feature = "encode")]
pub use plane_writer::encode;

#[cfg(feature = "alpha-benchmark-internals")]
pub use encode_token_output::BenchmarkWriterVariant;
#[cfg(feature = "alpha-benchmark-internals")]
pub use encode_token_output::set_benchmark_writer_variant;
