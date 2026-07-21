#![forbid(unsafe_code)]
//! VP8 lossy WebP decoder primitives.

mod bitstream;
mod coefficients;
mod entropy;
mod frame;
mod intra;
mod loop_filter;
mod partition;
mod quantization;
mod reconstruction;
#[cfg(test)]
mod test_support;
mod transform;

pub use bitstream::BoolDecoder;
pub use coefficients::COEFFICIENT_ZIGZAG;
pub use entropy::{
    CoefficientBlockType, CoefficientProbabilities, DecodedCoefficients, MacroblockResiduals,
    ResidualContext, decode_coefficients, decode_intra_residuals,
};
pub use frame::{Vp8YuvImage, decode_intra_frame};
pub use intra::{
    ChromaMode, Intra4Mode, Intra16Mode, IntraMacroblock, LumaMode, parse_intra_mode_row,
};
pub use loop_filter::{
    LoopFilterStrength, derive_loop_filter_strengths, filter_normal_edge, filter_simple_edge,
};
pub use partition::{
    FilterHeader, FirstPartitionHeader, PartitionLayout, SegmentHeader, TokenPartition, Vp8Header,
    parse_partition_layout, parse_riff_payload,
};
pub use quantization::{DequantizationMatrix, QuantizationHeader, derive_dequantization};
pub use reconstruction::{
    DequantizedMacroblock, MacroblockPixels, MacroblockPredictionEdges, MacroblockSpatialResidues,
    add_residue_and_clip, combine_macroblock_prediction, dequantize_macroblock,
    inverse_transform_macroblock, predict_intra4_block, predict_intra4_macroblock,
    predict_intra16_macroblock, reconstruct_intra_macroblock,
};
pub use transform::{inverse_dct_4x4, inverse_dct_4x4_i32, inverse_wht_4x4, inverse_wht_4x4_i32};
