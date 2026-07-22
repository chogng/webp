//! VP8L codec ownership.

mod allocation;
#[allow(dead_code)] // Retained reference helpers are exercised by codec-local tests.
mod backward_references;
#[allow(dead_code)] // Retained cache sink helpers are exercised by codec-local tests.
mod color_cache;
#[allow(dead_code)] // Header-only helpers remain useful to fuzzing and tests.
pub(crate) mod header;
#[allow(dead_code)] // Table introspection is retained for codec-local tests.
mod huffman;
#[allow(dead_code)] // Alternate bounded entry points remain available to fuzzing.
pub(crate) mod image_reader;
mod image_stream;
pub(crate) mod image_writer;
mod pixel;
mod transforms;
