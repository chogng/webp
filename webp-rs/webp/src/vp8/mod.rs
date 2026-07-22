#![forbid(unsafe_code)]
//! VP8 lossy WebP decoder primitives.

mod bool_coder;
mod coefficients;
#[cfg(feature = "encode")]
mod frame_writer;

mod frame_reader;
#[cfg(feature = "fuzzing")]
pub(crate) mod fuzzing;
mod intra_prediction;
mod loop_filter;
mod partitions;
mod quantization;
mod reconstruction;
mod row_decoder;
#[cfg(feature = "encode")]
mod sharp_yuv;
#[cfg(test)]
mod test_support;
mod transforms;
#[cfg(feature = "encode")]
mod yuv_image;

pub use bool_coder::BoolDecoder;
pub(crate) use bool_coder::BoolDecoderState;
pub use bool_coder::BoolEncodeError;
pub use bool_coder::BoolEncoder;
pub use coefficients::COEFFICIENT_ZIGZAG;
pub use coefficients::CoefficientBlockType;
#[cfg(feature = "encode")]
pub use coefficients::CoefficientEncodeError;
pub use coefficients::CoefficientProbabilities;
pub use coefficients::DecodedCoefficients;
pub use coefficients::MacroblockResiduals;
pub use coefficients::ResidualContext;
pub use coefficients::decode_coefficients;
pub use coefficients::decode_intra_residuals;
#[cfg(feature = "encode")]
pub use coefficients::encode_coefficients;
pub use frame_reader::Vp8YuvImage;
pub use frame_reader::decode_intra_frame;
#[cfg(feature = "encode")]
pub use frame_writer::Vp8DcMacroblockCoefficients;
#[cfg(feature = "encode")]
pub use frame_writer::Vp8EncodeError;
#[cfg(feature = "encode")]
pub use frame_writer::encode_dc_predicted_key_frame_with_quantizer;
#[cfg(feature = "encode")]
pub use frame_writer::encode_dc_predicted_macroblock_key_frame;
#[cfg(feature = "encode")]
pub use frame_writer::encode_dc_predicted_macroblock_key_frame_with_quantizer;
#[cfg(feature = "encode")]
pub use frame_writer::encode_neutral_key_frame;
#[cfg(feature = "encode")]
pub use frame_writer::quantize_dc_macroblock;
pub use intra_prediction::ChromaMode;
pub use intra_prediction::Intra4Mode;
pub use intra_prediction::Intra16Mode;
pub use intra_prediction::IntraMacroblock;
pub use intra_prediction::LumaMode;
pub use intra_prediction::parse_intra_mode_row;
pub use loop_filter::LoopFilterStrength;
pub use loop_filter::derive_loop_filter_strengths;
pub use loop_filter::filter_normal_edge;
pub use loop_filter::filter_simple_edge;
pub use partitions::FilterHeader;
pub use partitions::FirstPartitionHeader;
pub(crate) use partitions::IncrementalPartitionLayout;
pub use partitions::PartitionLayout;
pub use partitions::SegmentHeader;
pub use partitions::TokenPartition;
pub use partitions::Vp8Header;
pub(crate) use partitions::parse_incremental_partition_layout;
pub use partitions::parse_partition_layout;
pub use partitions::parse_riff_payload;
pub(crate) use partitions::parse_riff_payload_prefix;
pub use quantization::DequantizationMatrix;
pub use quantization::QuantizationHeader;
pub use quantization::derive_dequantization;
#[cfg(feature = "encode")]
pub use quantization::quantize_block;
pub use reconstruction::DequantizedMacroblock;
pub use reconstruction::MacroblockPixels;
pub use reconstruction::MacroblockPredictionEdges;
pub use reconstruction::MacroblockSpatialResidues;
pub use reconstruction::add_residue_and_clip;
pub use reconstruction::combine_macroblock_prediction;
pub use reconstruction::dequantize_macroblock;
pub use reconstruction::inverse_transform_macroblock;
pub use reconstruction::predict_intra4_block;
pub use reconstruction::predict_intra4_macroblock;
pub use reconstruction::predict_intra16_macroblock;
pub use reconstruction::reconstruct_intra_macroblock;
pub(crate) use row_decoder::IncrementalVp8Decoder;
#[cfg(feature = "encode")]
pub use transforms::forward_dct_4x4;
#[cfg(feature = "encode")]
pub use transforms::forward_dct_4x4_i32;
#[cfg(feature = "encode")]
pub use transforms::forward_wht_4x4;
#[cfg(feature = "encode")]
pub use transforms::forward_wht_4x4_i32;
pub use transforms::inverse_dct_4x4;
pub use transforms::inverse_dct_4x4_i32;
pub use transforms::inverse_wht_4x4;
pub use transforms::inverse_wht_4x4_i32;
#[cfg(feature = "encode")]
pub use yuv_image::Vp8SourceYuv;
#[cfg(feature = "encode")]
pub use yuv_image::rgba_to_yuv420;
