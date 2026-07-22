//! VP8 key-frame writer and source conversion.
//!
//! This module owns lossy VP8 output state: source-plane preparation and
//! frame partition construction. It uses the decoder crate's explicitly
//! unstable `vp8_codec` surface only for format primitives shared with the
//! VP8 reader.

mod frame_writer;
mod yuv_image;

pub use frame_writer::Vp8EncodeError;
pub use frame_writer::encode_dc_predicted_key_frame_with_quantizer;
pub use yuv_image::Vp8SourceYuv;
pub use yuv_image::rgba_to_yuv420;
