//! VP8 scalar residue and macroblock reconstruction kernels.

#[cfg(test)]
#[path = "vp8_reconstruction_tests.rs"]
mod tests;

use crate::vp8_prediction::Intra4Mode;
use crate::vp8_prediction::MacroblockPixels;
use crate::vp8_prediction::MacroblockPredictionEdges;
use crate::vp8_prediction::predict_intra4_block;

/// Spatial-domain signed residues for one VP8 macroblock before prediction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MacroblockSpatialResidues {
    pub luma: [[i32; 16]; 16],
    pub u: [[i32; 16]; 4],
    pub v: [[i32; 16]; 4],
}

/// Adds inverse-transform residues to a predicted macroblock and clips samples.
#[must_use]
pub fn combine_macroblock_prediction(
    mut prediction: MacroblockPixels,
    residues: MacroblockSpatialResidues,
) -> MacroblockPixels {
    combine_plane_blocks(&mut prediction.y, 16, 4, residues.luma);
    combine_plane_blocks(&mut prediction.u, 8, 2, residues.u);
    combine_plane_blocks(&mut prediction.v, 8, 2, residues.v);
    prediction
}

/// Adds sixteen 4×4 residue blocks to a 16×16 luma prediction.
#[must_use]
pub fn combine_luma_prediction(mut prediction: [u8; 256], residues: [[i32; 16]; 16]) -> [u8; 256] {
    combine_plane_blocks(&mut prediction, 16, 4, residues);
    prediction
}

/// Adds four 4×4 residue blocks to an 8×8 chroma prediction.
#[must_use]
pub fn combine_chroma_prediction(mut prediction: [u8; 64], residues: [[i32; 16]; 4]) -> [u8; 64] {
    combine_plane_blocks(&mut prediction, 8, 2, residues);
    prediction
}

/// Builds the luma prediction plane for one VP8 B_PRED macroblock.
#[must_use]
pub fn predict_intra4_macroblock(
    modes: [Intra4Mode; 16],
    edges: MacroblockPredictionEdges,
) -> [u8; 256] {
    reconstruct_intra4_luma(modes, edges, [[0; 16]; 16])
}

/// Reconstructs B_PRED luma blocks in raster order.
///
/// Each block's residue-adjusted samples become the prediction neighbours of
/// later blocks, matching VP8 decoder and encoder reconstruction.
#[must_use]
pub fn reconstruct_intra4_luma(
    modes: [Intra4Mode; 16],
    edges: MacroblockPredictionEdges,
    residues: [[i32; 16]; 16],
) -> [u8; 256] {
    let top_boundary = edges.top_y.unwrap_or([127; 16]);
    let left_boundary = edges.left_y.unwrap_or([129; 16]);
    let top_right = edges.top_right_y.unwrap_or([top_boundary[15]; 4]);
    let top_left = if edges.top_y.is_none() {
        127
    } else if edges.left_y.is_none() {
        129
    } else {
        edges.top_left_y
    };
    let mut output = [0_u8; 256];
    for (block_index, mode) in modes.into_iter().enumerate() {
        let block_x = (block_index % 4) * 4;
        let block_y = (block_index / 4) * 4;
        let top = std::array::from_fn(|index| {
            let x = block_x + index;
            if x >= 16 {
                top_right[x - 16]
            } else if block_y == 0 {
                top_boundary[x]
            } else {
                output[(block_y - 1) * 16 + x]
            }
        });
        let left = std::array::from_fn(|index| {
            let y = block_y + index;
            if block_x == 0 {
                left_boundary[y]
            } else {
                output[y * 16 + block_x - 1]
            }
        });
        let block_top_left = if block_x == 0 {
            if block_y == 0 {
                top_left
            } else {
                left_boundary[block_y - 1]
            }
        } else if block_y == 0 {
            top_boundary[block_x - 1]
        } else {
            output[(block_y - 1) * 16 + block_x - 1]
        };
        let prediction = predict_intra4_block(mode, block_top_left, top, left);
        for row in 0..4 {
            for column in 0..4 {
                let index = (block_y + row) * 16 + block_x + column;
                output[index] = add_residue_and_clip(
                    prediction[row * 4 + column],
                    residues[block_index][row * 4 + column],
                );
            }
        }
    }
    output
}

fn combine_plane_blocks<const PIXELS: usize, const BLOCKS: usize>(
    plane: &mut [u8; PIXELS],
    stride: usize,
    blocks_per_row: usize,
    blocks: [[i32; 16]; BLOCKS],
) {
    for (block_index, block) in blocks.into_iter().enumerate() {
        let block_x = (block_index % blocks_per_row) * 4;
        let block_y = (block_index / blocks_per_row) * 4;
        for row in 0..4 {
            for column in 0..4 {
                let destination = (block_y + row) * stride + block_x + column;
                plane[destination] =
                    add_residue_and_clip(plane[destination], block[row * 4 + column]);
            }
        }
    }
}

/// Adds one signed VP8 residue to a prediction sample with saturating clip.
#[must_use]
pub fn add_residue_and_clip(prediction: u8, residue: i32) -> u8 {
    i64::from(prediction)
        .saturating_add(i64::from(residue))
        .clamp(0, 255) as u8
}
