#![forbid(unsafe_code)]
//! A bounded decoder for static VP8L images.
//!
//! The decoder supports VP8L's four transforms, color cache, literal and
//! backward-reference entropy symbols, and spatial meta-Huffman groups. The
//! output uses straight RGBA byte order.

mod allocation;
#[cfg(test)]
mod decode_profile;
mod huffman_group;
mod image;
mod image_data;
mod inverse_color;
mod inverse_indexing;
mod inverse_predictor;
mod pixel;
mod pixel_buffer;
mod pixel_output;
mod transform_list;

pub use image::LiteralImage;
pub use image::decode_literal_only;
pub use image::decode_no_transform;
pub use image::decode_vp8l;
