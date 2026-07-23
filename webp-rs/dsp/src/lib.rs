#![forbid(unsafe_code)]
//! Pure WebP pixel-domain kernels shared by decoding and encoding.

mod vp8_loop_filter;
mod vp8_prediction;
mod vp8_reconstruction;
mod vp8_transforms;

pub use vp8_loop_filter::LoopFilterStrength;
pub use vp8_loop_filter::filter_normal_edge;
pub use vp8_loop_filter::filter_simple_edge;
pub use vp8_prediction::ChromaMode;
pub use vp8_prediction::Intra4Mode;
pub use vp8_prediction::Intra16Mode;
pub use vp8_prediction::MacroblockPixels;
pub use vp8_prediction::MacroblockPredictionEdges;
pub use vp8_prediction::predict_intra4_block;
pub use vp8_prediction::predict_intra16_macroblock;
pub use vp8_reconstruction::MacroblockSpatialResidues;
pub use vp8_reconstruction::add_residue_and_clip;
pub use vp8_reconstruction::combine_chroma_prediction;
pub use vp8_reconstruction::combine_luma_prediction;
pub use vp8_reconstruction::combine_macroblock_prediction;
pub use vp8_reconstruction::predict_intra4_macroblock;
pub use vp8_reconstruction::reconstruct_intra4_luma;

pub use vp8_transforms::forward_dct_4x4;
pub use vp8_transforms::forward_dct_4x4_i32;
pub use vp8_transforms::forward_wht_4x4;
pub use vp8_transforms::forward_wht_4x4_i32;
pub use vp8_transforms::inverse_dct_4x4;
pub use vp8_transforms::inverse_dct_4x4_i32;
pub use vp8_transforms::inverse_wht_4x4;
pub use vp8_transforms::inverse_wht_4x4_i32;
