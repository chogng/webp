//! VP8 intra-macroblock coefficient reconstruction.
//!
//! This module adapts entropy-domain residuals and dequantization controls to
//! the stateless prediction and sample kernels owned by `webp-dsp`.

use crate::DecodeError;

use crate::vp8::ChromaMode;
use crate::vp8::DequantizationMatrix;
#[cfg(test)]
use crate::vp8::Intra4Mode;
use crate::vp8::Intra16Mode;
use crate::vp8::IntraMacroblock;
use crate::vp8::LumaMode;
use crate::vp8::MacroblockResiduals;
pub use webp_dsp::MacroblockPixels;
pub use webp_dsp::MacroblockPredictionEdges;
pub use webp_dsp::MacroblockSpatialResidues;
pub use webp_dsp::add_residue_and_clip;
use webp_dsp::combine_chroma_prediction;
use webp_dsp::combine_luma_prediction;
pub use webp_dsp::combine_macroblock_prediction;
use webp_dsp::inverse_dct_4x4_i32;
use webp_dsp::inverse_wht_4x4_i32;
pub use webp_dsp::predict_intra4_block;
pub use webp_dsp::predict_intra4_macroblock;
pub use webp_dsp::predict_intra16_macroblock;
use webp_dsp::reconstruct_intra4_luma;

#[cfg(test)]
#[path = "reconstruction_tests.rs"]
mod tests;

/// Dequantized frequency-domain coefficients for one VP8 macroblock.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DequantizedMacroblock {
    pub luma: [[i32; 16]; 16],
    pub u: [[i32; 16]; 4],
    pub v: [[i32; 16]; 4],
}

/// Applies one segment's VP8 dequantization matrix to a macroblock.
#[must_use]
pub fn dequantize_macroblock(
    residuals: &MacroblockResiduals,
    matrix: DequantizationMatrix,
) -> DequantizedMacroblock {
    let mut luma = residuals
        .luma
        .map(|block| dequantize_block(block.values, matrix.y1_dc, matrix.y1_ac));
    if let Some(y2) = residuals.y2 {
        let y2_values = dequantize_block(y2.values, matrix.y2_dc, matrix.y2_ac);
        for (block, dc) in luma.iter_mut().zip(inverse_wht_4x4_i32(y2_values)) {
            block[0] = dc;
        }
    }
    DequantizedMacroblock {
        luma,
        u: residuals
            .u
            .map(|block| dequantize_block(block.values, matrix.uv_dc, matrix.uv_ac)),
        v: residuals
            .v
            .map(|block| dequantize_block(block.values, matrix.uv_dc, matrix.uv_ac)),
    }
}

/// Applies VP8's inverse 4×4 DCT to every dequantized macroblock block.
#[must_use]
pub fn inverse_transform_macroblock(
    coefficients: DequantizedMacroblock,
) -> MacroblockSpatialResidues {
    MacroblockSpatialResidues {
        luma: coefficients.luma.map(inverse_dct_4x4_i32),
        u: coefficients.u.map(inverse_dct_4x4_i32),
        v: coefficients.v.map(inverse_dct_4x4_i32),
    }
}

/// Reconstructs one complete VP8 intra macroblock from entropy tokens.
pub fn reconstruct_intra_macroblock(
    block: IntraMacroblock,
    residuals: &MacroblockResiduals,
    matrix: DequantizationMatrix,
    edges: MacroblockPredictionEdges,
) -> Result<MacroblockPixels, DecodeError> {
    let spatial = inverse_transform_macroblock(dequantize_macroblock(residuals, matrix));
    let prediction = match block.luma {
        LumaMode::Sixteen(mode) => predict_intra16_macroblock(mode, block.chroma, edges),
        LumaMode::FourByFour(modes) => {
            let mut prediction = predict_intra16_macroblock(Intra16Mode::Dc, block.chroma, edges);
            prediction.y = reconstruct_intra4_luma(modes, edges, spatial.luma);
            prediction.u = combine_chroma_prediction(prediction.u, spatial.u);
            prediction.v = combine_chroma_prediction(prediction.v, spatial.v);
            return Ok(prediction);
        }
    };
    Ok(combine_macroblock_prediction(prediction, spatial))
}

pub fn reconstruct_intra16_luma(
    mode: Intra16Mode,
    residuals: &MacroblockResiduals,
    matrix: DequantizationMatrix,
    edges: MacroblockPredictionEdges,
) -> [u8; 256] {
    let mut coefficients = residuals
        .luma
        .map(|block| dequantize_block(block.values, matrix.y1_dc, matrix.y1_ac));
    if let Some(y2) = residuals.y2 {
        let y2_values = dequantize_block(y2.values, matrix.y2_dc, matrix.y2_ac);
        for (block, dc) in coefficients.iter_mut().zip(inverse_wht_4x4_i32(y2_values)) {
            block[0] = dc;
        }
    }
    let residues = coefficients.map(inverse_dct_4x4_i32);
    let prediction = predict_intra16_macroblock(mode, ChromaMode::Dc, edges).y;
    combine_luma_prediction(prediction, residues)
}

pub fn reconstruct_intra16_chroma(
    mode: ChromaMode,
    residuals: &MacroblockResiduals,
    matrix: DequantizationMatrix,
    edges: MacroblockPredictionEdges,
) -> ([u8; 64], [u8; 64]) {
    let prediction = predict_intra16_macroblock(Intra16Mode::Dc, mode, edges);
    let u = residuals
        .u
        .map(|block| dequantize_block(block.values, matrix.uv_dc, matrix.uv_ac))
        .map(inverse_dct_4x4_i32);
    let v = residuals
        .v
        .map(|block| dequantize_block(block.values, matrix.uv_dc, matrix.uv_ac))
        .map(inverse_dct_4x4_i32);
    (
        combine_chroma_prediction(prediction.u, u),
        combine_chroma_prediction(prediction.v, v),
    )
}

fn dequantize_block(values: [i16; 16], dc: u16, ac: u16) -> [i32; 16] {
    let mut output = values.map(|value| i32::from(value) * i32::from(ac));
    output[0] = i32::from(values[0]) * i32::from(dc);
    output
}
