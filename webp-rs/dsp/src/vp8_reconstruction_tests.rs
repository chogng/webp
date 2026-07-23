//! Tests for VP8 residue and macroblock reconstruction kernels.

use super::*;
use crate::Intra4Mode;

#[test]
fn macroblock_sample_composition_maps_blocks_and_clips_edges() {
    let mut residues = MacroblockSpatialResidues {
        luma: [[0; 16]; 16],
        u: [[0; 16]; 4],
        v: [[0; 16]; 4],
    };
    residues.luma[0][0] = 2;
    residues.luma[5][6] = -3;
    residues.u[3][15] = 200;
    residues.v[0][0] = -200;
    let pixels = combine_macroblock_prediction(
        MacroblockPixels {
            y: [128; 256],
            u: [128; 64],
            v: [128; 64],
        },
        residues,
    );
    assert_eq!(pixels.y[0], 130);
    assert_eq!(pixels.y[5 * 16 + 6], 125);
    assert_eq!(pixels.u[7 * 8 + 7], 255);
    assert_eq!(pixels.v[0], 0);
    assert_eq!(add_residue_and_clip(0, i32::MIN), 0);
    assert_eq!(add_residue_and_clip(255, i32::MAX), 255);
}

#[test]
fn intra4_reconstruction_uses_residue_adjusted_raster_neighbours() {
    let edges = MacroblockPredictionEdges {
        top_y: Some([10; 16]),
        top_right_y: Some([10; 4]),
        left_y: Some([30; 16]),
        top_left_y: 5,
        ..MacroblockPredictionEdges::default()
    };
    let prediction = predict_intra4_macroblock([Intra4Mode::Dc; 16], edges);
    assert_eq!(prediction[0], 20);
    assert_eq!(prediction[4], 15);

    let mut residues = [[0_i32; 16]; 16];
    residues[0].fill(16);
    let reconstructed = reconstruct_intra4_luma(
        [Intra4Mode::Horizontal; 16],
        MacroblockPredictionEdges::default(),
        residues,
    );
    assert_eq!(reconstructed[0], 145);
    assert!(reconstructed[4] > 129);
}
