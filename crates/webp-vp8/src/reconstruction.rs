//! VP8 intra-macroblock reconstruction.
//!
//! This module owns the pure pixel-domain work after entropy decoding:
//! dequantization, inverse transforms, prediction, and residue application.
//! Frame orchestration and bitstream parsing stay in their dedicated layers.

use webp_core::DecodeError;

use crate::transform::{inverse_dct_4x4_i32, inverse_wht_4x4_i32};
use crate::{
    ChromaMode, DequantizationMatrix, DequantizedMacroblock, Intra4Mode, Intra16Mode,
    IntraMacroblock, LumaMode, MacroblockPixels, MacroblockPredictionEdges, MacroblockResiduals,
    MacroblockSpatialResidues,
};

/// Applies one segment's VP8 dequantization matrix to a macroblock.
///
/// For a 16×16-predicted luma macroblock, this also inverse-transforms the
/// Y2 block and places its sixteen DC values into the luma blocks, matching
/// VP8's coefficient layout. All output is widened to `i32`.
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

/// Adds inverse-transform residues to a predicted macroblock and clips YUV
/// samples to the valid `0..=255` range.
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

/// Builds a 16×16-luma/8×8-chroma VP8 intra prediction for non-B_PRED luma.
///
/// VP8 initializes unavailable top and left neighbours to its 127 and 129
/// sentinel values, respectively. DC prediction retains its separate edge
/// averaging rules.
#[must_use]
pub fn predict_intra16_macroblock(
    luma_mode: Intra16Mode,
    chroma_mode: ChromaMode,
    edges: MacroblockPredictionEdges,
) -> MacroblockPixels {
    let mut prediction = MacroblockPixels {
        y: [0; 256],
        u: [0; 64],
        v: [0; 64],
    };
    predict_plane(
        &mut prediction.y,
        luma_mode.into(),
        edges.top_y,
        edges.left_y,
        edges.top_left_y,
    );
    predict_plane(
        &mut prediction.u,
        chroma_mode.into(),
        edges.top_u,
        edges.left_u,
        edges.top_left_u,
    );
    predict_plane(
        &mut prediction.v,
        chroma_mode.into(),
        edges.top_v,
        edges.left_v,
        edges.top_left_v,
    );
    prediction
}

/// Builds the luma prediction plane for one VP8 B_PRED macroblock.
///
/// Blocks are predicted in raster order, so every block after the first reads
/// reconstructed samples written by its earlier neighbours. At a picture edge
/// VP8 uses 127 top and 129 left sentinel samples; absent top-right samples
/// replicate the final top sample, matching the rightmost-macroblock rule.
#[must_use]
pub fn predict_intra4_macroblock(
    modes: [Intra4Mode; 16],
    edges: MacroblockPredictionEdges,
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
        let block = predict_intra4_block(mode, block_top_left, top, left);
        for row in 0..4 {
            output[(block_y + row) * 16 + block_x..(block_y + row) * 16 + block_x + 4]
                .copy_from_slice(&block[row * 4..row * 4 + 4]);
        }
    }
    output
}

/// Reconstructs one complete VP8 intra macroblock from entropy tokens.
///
/// This combines segment dequantization, inverse transforms, intra prediction,
/// and sample clipping. Macroblock-row orchestration owns the supplied edge
/// cache and calls this once mode and residual token parsing are complete.
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
            combine_plane_blocks(&mut prediction.u, 8, 2, spatial.u);
            combine_plane_blocks(&mut prediction.v, 8, 2, spatial.v);
            return Ok(prediction);
        }
    };
    Ok(combine_macroblock_prediction(prediction, spatial))
}

fn reconstruct_intra4_luma(
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

#[derive(Clone, Copy)]
enum PlanePredictionMode {
    Dc,
    Vertical,
    Horizontal,
    TrueMotion,
}

impl From<Intra16Mode> for PlanePredictionMode {
    fn from(mode: Intra16Mode) -> Self {
        match mode {
            Intra16Mode::Dc => Self::Dc,
            Intra16Mode::Vertical => Self::Vertical,
            Intra16Mode::Horizontal => Self::Horizontal,
            Intra16Mode::TrueMotion => Self::TrueMotion,
        }
    }
}

impl From<ChromaMode> for PlanePredictionMode {
    fn from(mode: ChromaMode) -> Self {
        match mode {
            ChromaMode::Dc => Self::Dc,
            ChromaMode::Vertical => Self::Vertical,
            ChromaMode::Horizontal => Self::Horizontal,
            ChromaMode::TrueMotion => Self::TrueMotion,
        }
    }
}

fn predict_plane<const SIZE: usize>(
    output: &mut [u8],
    mode: PlanePredictionMode,
    top: Option<[u8; SIZE]>,
    left: Option<[u8; SIZE]>,
    top_left: u8,
) {
    debug_assert_eq!(output.len(), SIZE * SIZE);
    match mode {
        PlanePredictionMode::Dc => {
            let value = match (top, left) {
                (Some(top), Some(left)) => {
                    let sum = top.into_iter().map(u32::from).sum::<u32>()
                        + left.into_iter().map(u32::from).sum::<u32>();
                    ((sum + SIZE as u32) / (2 * SIZE) as u32) as u8
                }
                (Some(top), None) => {
                    ((top.into_iter().map(u32::from).sum::<u32>() + (SIZE / 2) as u32)
                        / SIZE as u32) as u8
                }
                (None, Some(left)) => {
                    ((left.into_iter().map(u32::from).sum::<u32>() + (SIZE / 2) as u32)
                        / SIZE as u32) as u8
                }
                (None, None) => 128,
            };
            output.fill(value);
        }
        PlanePredictionMode::Vertical => {
            let top = top.unwrap_or([127; SIZE]);
            for row in output.chunks_exact_mut(SIZE) {
                row.copy_from_slice(&top);
            }
        }
        PlanePredictionMode::Horizontal => {
            let left = left.unwrap_or([129; SIZE]);
            for (row, &value) in output.chunks_exact_mut(SIZE).zip(left.iter()) {
                row.fill(value);
            }
        }
        PlanePredictionMode::TrueMotion => {
            let top_left = match (top, left) {
                (None, _) => 127,
                (Some(_), None) => 129,
                (Some(_), Some(_)) => top_left,
            };
            let top = top.unwrap_or([127; SIZE]);
            let left = left.unwrap_or([129; SIZE]);
            for (row, &left_value) in output.chunks_exact_mut(SIZE).zip(left.iter()) {
                for (sample, &top_value) in row.iter_mut().zip(top.iter()) {
                    *sample = (i32::from(left_value) + i32::from(top_value) - i32::from(top_left))
                        .clamp(0, 255) as u8;
                }
            }
        }
    }
}

/// Predicts one VP8 B_PRED luma 4×4 block from its already-reconstructed
/// neighbours. `top` supplies the four direct and four top-right samples.
#[must_use]
pub fn predict_intra4_block(
    mode: Intra4Mode,
    top_left: u8,
    top: [u8; 8],
    left: [u8; 4],
) -> [u8; 16] {
    let mut out = [0_u8; 16];
    let set = |out: &mut [u8; 16], x: usize, y: usize, value: u8| out[y * 4 + x] = value;
    let a2 = |a: u8, b: u8| ((u16::from(a) + u16::from(b) + 1) >> 1) as u8;
    let a3 =
        |a: u8, b: u8, c: u8| ((u16::from(a) + 2 * u16::from(b) + u16::from(c) + 2) >> 2) as u8;
    match mode {
        Intra4Mode::Dc => {
            let value = (top[..4]
                .iter()
                .chain(left.iter())
                .map(|&value| u16::from(value))
                .sum::<u16>()
                + 4)
                >> 3;
            out.fill(value as u8);
        }
        Intra4Mode::TrueMotion => {
            for (y, &left_value) in left.iter().enumerate() {
                for (x, &top_value) in top[..4].iter().enumerate() {
                    set(
                        &mut out,
                        x,
                        y,
                        (i32::from(left_value) + i32::from(top_value) - i32::from(top_left))
                            .clamp(0, 255) as u8,
                    );
                }
            }
        }
        Intra4Mode::Vertical => {
            let row = [
                a3(top_left, top[0], top[1]),
                a3(top[0], top[1], top[2]),
                a3(top[1], top[2], top[3]),
                a3(top[2], top[3], top[4]),
            ];
            for y in 0..4 {
                out[y * 4..y * 4 + 4].copy_from_slice(&row);
            }
        }
        Intra4Mode::Horizontal => {
            let rows = [
                a3(top_left, left[0], left[1]),
                a3(left[0], left[1], left[2]),
                a3(left[1], left[2], left[3]),
                a3(left[2], left[3], left[3]),
            ];
            for (y, value) in rows.into_iter().enumerate() {
                out[y * 4..y * 4 + 4].fill(value);
            }
        }
        Intra4Mode::DiagonalDownRight => {
            set(&mut out, 0, 3, a3(left[1], left[2], left[3]));
            for (x, y) in [(1, 3), (0, 2)] {
                set(&mut out, x, y, a3(left[0], left[1], left[2]));
            }
            for (x, y) in [(2, 3), (1, 2), (0, 1)] {
                set(&mut out, x, y, a3(top_left, left[0], left[1]));
            }
            for (x, y) in [(3, 3), (2, 2), (1, 1), (0, 0)] {
                set(&mut out, x, y, a3(top[0], top_left, left[0]));
            }
            for (x, y) in [(3, 2), (2, 1), (1, 0)] {
                set(&mut out, x, y, a3(top[1], top[0], top_left));
            }
            for (x, y) in [(3, 1), (2, 0)] {
                set(&mut out, x, y, a3(top[2], top[1], top[0]));
            }
            set(&mut out, 3, 0, a3(top[3], top[2], top[1]));
        }
        Intra4Mode::DiagonalDownLeft => {
            set(&mut out, 0, 0, a3(top[0], top[1], top[2]));
            for (x, y) in [(1, 0), (0, 1)] {
                set(&mut out, x, y, a3(top[1], top[2], top[3]));
            }
            for (x, y) in [(2, 0), (1, 1), (0, 2)] {
                set(&mut out, x, y, a3(top[2], top[3], top[4]));
            }
            for (x, y) in [(3, 0), (2, 1), (1, 2), (0, 3)] {
                set(&mut out, x, y, a3(top[3], top[4], top[5]));
            }
            for (x, y) in [(3, 1), (2, 2), (1, 3)] {
                set(&mut out, x, y, a3(top[4], top[5], top[6]));
            }
            for (x, y) in [(3, 2), (2, 3)] {
                set(&mut out, x, y, a3(top[5], top[6], top[7]));
            }
            set(&mut out, 3, 3, a3(top[6], top[7], top[7]));
        }
        Intra4Mode::VerticalRight => {
            for (x, value) in [
                a2(top_left, top[0]),
                a2(top[0], top[1]),
                a2(top[1], top[2]),
                a2(top[2], top[3]),
            ]
            .into_iter()
            .enumerate()
            {
                set(&mut out, x, 0, value);
            }
            set(&mut out, 0, 3, a3(left[2], left[1], left[0]));
            set(&mut out, 0, 2, a3(left[1], left[0], top_left));
            for (x, y) in [(0, 1), (1, 3)] {
                set(&mut out, x, y, a3(left[0], top_left, top[0]));
            }
            for (x, y) in [(1, 1), (2, 3)] {
                set(&mut out, x, y, a3(top_left, top[0], top[1]));
            }
            for (x, y) in [(2, 1), (3, 3)] {
                set(&mut out, x, y, a3(top[0], top[1], top[2]));
            }
            set(&mut out, 3, 1, a3(top[1], top[2], top[3]));
            for (x, y, value) in [
                (1, 2, a2(top_left, top[0])),
                (2, 2, a2(top[0], top[1])),
                (3, 2, a2(top[1], top[2])),
            ] {
                set(&mut out, x, y, value);
            }
        }
        Intra4Mode::VerticalLeft => {
            for (x, value) in [
                a2(top[0], top[1]),
                a2(top[1], top[2]),
                a2(top[2], top[3]),
                a2(top[3], top[4]),
            ]
            .into_iter()
            .enumerate()
            {
                set(&mut out, x, 0, value);
            }
            for (x, y, value) in [
                (0, 2, a2(top[1], top[2])),
                (1, 2, a2(top[2], top[3])),
                (2, 2, a2(top[3], top[4])),
            ] {
                set(&mut out, x, y, value);
            }
            set(&mut out, 0, 1, a3(top[0], top[1], top[2]));
            for (x, y) in [(1, 1), (0, 3)] {
                set(&mut out, x, y, a3(top[1], top[2], top[3]));
            }
            for (x, y) in [(2, 1), (1, 3)] {
                set(&mut out, x, y, a3(top[2], top[3], top[4]));
            }
            for (x, y) in [(3, 1), (2, 3)] {
                set(&mut out, x, y, a3(top[3], top[4], top[5]));
            }
            set(&mut out, 3, 2, a3(top[4], top[5], top[6]));
            set(&mut out, 3, 3, a3(top[5], top[6], top[7]));
        }
        Intra4Mode::HorizontalUp => {
            set(&mut out, 0, 0, a2(left[0], left[1]));
            for (x, y) in [(2, 0), (0, 1)] {
                set(&mut out, x, y, a2(left[1], left[2]));
            }
            for (x, y) in [(2, 1), (0, 2)] {
                set(&mut out, x, y, a2(left[2], left[3]));
            }
            set(&mut out, 1, 0, a3(left[0], left[1], left[2]));
            for (x, y) in [(3, 0), (1, 1)] {
                set(&mut out, x, y, a3(left[1], left[2], left[3]));
            }
            for (x, y) in [(3, 1), (1, 2)] {
                set(&mut out, x, y, a3(left[2], left[3], left[3]));
            }
            for (x, y) in [(3, 2), (2, 2), (0, 3), (1, 3), (2, 3), (3, 3)] {
                set(&mut out, x, y, left[3]);
            }
        }
        Intra4Mode::HorizontalDown => {
            for (x, y) in [(0, 0), (2, 1)] {
                set(&mut out, x, y, a2(left[0], top_left));
            }
            for (x, y) in [(0, 1), (2, 2)] {
                set(&mut out, x, y, a2(left[1], left[0]));
            }
            for (x, y) in [(0, 2), (2, 3)] {
                set(&mut out, x, y, a2(left[2], left[1]));
            }
            set(&mut out, 0, 3, a2(left[3], left[2]));
            set(&mut out, 3, 0, a3(top[0], top[1], top[2]));
            set(&mut out, 2, 0, a3(top_left, top[0], top[1]));
            for (x, y) in [(1, 0), (3, 1)] {
                set(&mut out, x, y, a3(left[0], top_left, top[0]));
            }
            for (x, y) in [(1, 1), (3, 2)] {
                set(&mut out, x, y, a3(left[1], left[0], top_left));
            }
            for (x, y) in [(1, 2), (3, 3)] {
                set(&mut out, x, y, a3(left[2], left[1], left[0]));
            }
            set(&mut out, 1, 3, a3(left[3], left[2], left[1]));
        }
    }
    out
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
    (i32::from(prediction) + residue).clamp(0, 255) as u8
}

fn dequantize_block(values: [i16; 16], dc: u16, ac: u16) -> [i32; 16] {
    let mut output = values.map(|value| i32::from(value) * i32::from(ac));
    output[0] = i32::from(values[0]) * i32::from(dc);
    output
}
