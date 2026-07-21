use super::*;
use crate::DecodedCoefficients;

#[test]
fn widened_transforms_and_macroblock_dequantization_preserve_y2_dc_layout() {
    let mut dc = [0_i32; 16];
    dc[0] = 16;
    assert_eq!(inverse_dct_4x4_i32(dc), [2; 16]);

    let empty = DecodedCoefficients {
        values: [0; 16],
        end: 0,
        non_zero: 0,
    };
    let mut residuals = MacroblockResiduals {
        y2: Some(DecodedCoefficients {
            values: [8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            end: 1,
            non_zero: 1,
        }),
        luma: [empty; 16],
        u: [empty; 4],
        v: [empty; 4],
        non_zero_y: 0,
        non_zero_uv: 0,
    };
    residuals.luma[0].values[1] = 2;
    residuals.u[0].values[0] = 3;
    residuals.u[0].values[1] = -2;
    let matrix = DequantizationMatrix {
        y1_dc: 2,
        y1_ac: 3,
        y2_dc: 4,
        y2_ac: 5,
        uv_dc: 6,
        uv_ac: 7,
        uv_quant: 0,
    };

    let dequantized = dequantize_macroblock(&residuals, matrix);
    assert_eq!(dequantized.luma[0][0], 4);
    assert_eq!(dequantized.luma[15][0], 4);
    assert_eq!(dequantized.luma[0][1], 6);
    assert_eq!(dequantized.u[0][0], 18);
    assert_eq!(dequantized.u[0][1], -14);
    let spatial = inverse_transform_macroblock(dequantized);
    assert_eq!(spatial.luma[0], inverse_dct_4x4_i32(dequantized.luma[0]));
}

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
    assert_eq!(add_residue_and_clip(0, -1), 0);
    assert_eq!(add_residue_and_clip(255, 1), 255);
}

#[test]
fn intra16_prediction_uses_neighbours_and_dc_boundary_fallbacks() {
    let edges = MacroblockPredictionEdges {
        top_y: Some([10; 16]),
        top_right_y: Some([10; 4]),
        left_y: Some([30; 16]),
        top_left_y: 5,
        top_u: Some([50; 8]),
        left_u: Some([70; 8]),
        top_left_u: 20,
        top_v: Some([80; 8]),
        left_v: Some([90; 8]),
        top_left_v: 30,
    };
    let prediction =
        predict_intra16_macroblock(Intra16Mode::Vertical, ChromaMode::Horizontal, edges);
    assert_eq!(prediction.y, [10; 256]);
    assert_eq!(prediction.u, [70; 64]);
    assert_eq!(prediction.v, [90; 64]);

    let true_motion =
        predict_intra16_macroblock(Intra16Mode::TrueMotion, ChromaMode::TrueMotion, edges);
    assert_eq!(true_motion.y, [35; 256]);
    assert_eq!(true_motion.u, [100; 64]);
    assert_eq!(true_motion.v, [140; 64]);

    let dc = predict_intra16_macroblock(
        Intra16Mode::Dc,
        ChromaMode::Dc,
        MacroblockPredictionEdges::default(),
    );
    assert_eq!(dc.y, [128; 256]);
    assert_eq!(dc.u, [128; 64]);
    assert_eq!(dc.v, [128; 64]);
    let sentinel = predict_intra16_macroblock(
        Intra16Mode::Vertical,
        ChromaMode::Horizontal,
        MacroblockPredictionEdges::default(),
    );
    assert_eq!(sentinel.y, [127; 256]);
    assert_eq!(sentinel.u, [129; 64]);
}

#[test]
fn plane_specific_intra16_reconstruction_matches_full_macroblock() {
    let empty = DecodedCoefficients {
        values: [0; 16],
        end: 0,
        non_zero: 0,
    };
    let mut residuals = MacroblockResiduals {
        y2: Some(empty),
        luma: [empty; 16],
        u: [empty; 4],
        v: [empty; 4],
        non_zero_y: 0,
        non_zero_uv: 0,
    };
    residuals.y2.as_mut().unwrap().values[0] = 7;
    residuals.luma[3].values[1] = -4;
    residuals.u[1].values[0] = 5;
    residuals.v[2].values[2] = -3;
    let matrix = DequantizationMatrix {
        y1_dc: 4,
        y1_ac: 7,
        y2_dc: 8,
        y2_ac: 9,
        uv_dc: 5,
        uv_ac: 6,
        uv_quant: 0,
    };
    let edges = MacroblockPredictionEdges {
        top_y: Some(std::array::from_fn(|index| 31 + index as u8 * 7)),
        left_y: Some(std::array::from_fn(|index| {
            203_u8.wrapping_sub(index as u8 * 5)
        })),
        top_left_y: 91,
        top_u: Some(std::array::from_fn(|index| 51 + index as u8 * 8)),
        left_u: Some(std::array::from_fn(|index| {
            177_u8.wrapping_sub(index as u8 * 6)
        })),
        top_left_u: 99,
        top_v: Some(std::array::from_fn(|index| 39 + index as u8 * 10)),
        left_v: Some(std::array::from_fn(|index| {
            191_u8.wrapping_sub(index as u8 * 7)
        })),
        top_left_v: 87,
        ..MacroblockPredictionEdges::default()
    };
    for luma_mode in [
        Intra16Mode::Dc,
        Intra16Mode::Vertical,
        Intra16Mode::Horizontal,
        Intra16Mode::TrueMotion,
    ] {
        for chroma_mode in [
            ChromaMode::Dc,
            ChromaMode::Vertical,
            ChromaMode::Horizontal,
            ChromaMode::TrueMotion,
        ] {
            let full = reconstruct_intra_macroblock(
                IntraMacroblock {
                    segment: 0,
                    skip: false,
                    luma: LumaMode::Sixteen(luma_mode),
                    chroma: chroma_mode,
                },
                &residuals,
                matrix,
                edges,
            )
            .unwrap();
            assert_eq!(
                reconstruct_intra16_luma(luma_mode, &residuals, matrix, edges),
                full.y
            );
            let (u, v) = reconstruct_intra16_chroma(chroma_mode, &residuals, matrix, edges);
            assert_eq!((u, v), (full.u, full.v));
        }
    }
}

#[test]
fn intra4_prediction_covers_all_vp8_directional_modes() {
    let top = [10, 20, 30, 40, 50, 60, 70, 80];
    let left = [50, 60, 70, 80];
    let dc = predict_intra4_block(Intra4Mode::Dc, 5, top, left);
    assert_eq!(dc, [45; 16]);
    let true_motion = predict_intra4_block(Intra4Mode::TrueMotion, 5, top, left);
    assert_eq!(true_motion[0], 55);
    assert_eq!(true_motion[15], 115);
    assert_eq!(
        predict_intra4_block(Intra4Mode::Vertical, 5, top, left),
        [
            11, 20, 30, 40, 11, 20, 30, 40, 11, 20, 30, 40, 11, 20, 30, 40
        ]
    );
    assert_eq!(
        predict_intra4_block(Intra4Mode::Horizontal, 5, top, left),
        [
            41, 41, 41, 41, 60, 60, 60, 60, 70, 70, 70, 70, 78, 78, 78, 78
        ]
    );
    for mode in [
        Intra4Mode::DiagonalDownRight,
        Intra4Mode::VerticalRight,
        Intra4Mode::DiagonalDownLeft,
        Intra4Mode::VerticalLeft,
        Intra4Mode::HorizontalDown,
        Intra4Mode::HorizontalUp,
    ] {
        let prediction = predict_intra4_block(mode, 5, top, left);
        assert_ne!(prediction, [128; 16], "{mode:?}");
    }
    let diagonal_left = predict_intra4_block(Intra4Mode::DiagonalDownLeft, 5, top, left);
    assert_eq!(diagonal_left[0], 20);
    assert_eq!(diagonal_left[15], 78);
    let horizontal_up = predict_intra4_block(Intra4Mode::HorizontalUp, 5, top, left);
    assert_eq!(horizontal_up[12..], [80; 4]);
}

#[test]
fn intra4_macroblock_and_full_reconstruction_follow_raster_neighbours() {
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

    let empty = DecodedCoefficients {
        values: [0; 16],
        end: 0,
        non_zero: 0,
    };
    let residuals = MacroblockResiduals {
        y2: None,
        luma: [empty; 16],
        u: [empty; 4],
        v: [empty; 4],
        non_zero_y: 0,
        non_zero_uv: 0,
    };
    let pixels = reconstruct_intra_macroblock(
        IntraMacroblock {
            segment: 0,
            skip: true,
            luma: LumaMode::FourByFour([Intra4Mode::Dc; 16]),
            chroma: ChromaMode::Dc,
        },
        &residuals,
        DequantizationMatrix {
            y1_dc: 1,
            y1_ac: 1,
            y2_dc: 1,
            y2_ac: 1,
            uv_dc: 1,
            uv_ac: 1,
            uv_quant: 0,
        },
        MacroblockPredictionEdges::default(),
    )
    .unwrap();
    assert!(pixels.y[..64].iter().all(|&value| value == 128));
    assert!(pixels.y[64..].iter().all(|&value| value == 129));
    assert_eq!(pixels.u, [128; 64]);
    assert_eq!(pixels.v, [128; 64]);
}

#[test]
fn intra4_reconstruction_uses_residue_adjusted_left_neighbour() {
    let empty = DecodedCoefficients {
        values: [0; 16],
        end: 0,
        non_zero: 0,
    };
    let mut residuals = MacroblockResiduals {
        y2: None,
        luma: [empty; 16],
        u: [empty; 4],
        v: [empty; 4],
        non_zero_y: 0,
        non_zero_uv: 0,
    };
    residuals.luma[0].values[0] = 160;
    let pixels = reconstruct_intra_macroblock(
        IntraMacroblock {
            segment: 0,
            skip: false,
            luma: LumaMode::FourByFour([Intra4Mode::Horizontal; 16]),
            chroma: ChromaMode::Dc,
        },
        &residuals,
        DequantizationMatrix {
            y1_dc: 1,
            y1_ac: 1,
            y2_dc: 1,
            y2_ac: 1,
            uv_dc: 1,
            uv_ac: 1,
            uv_quant: 0,
        },
        MacroblockPredictionEdges::default(),
    )
    .unwrap();
    assert!(pixels.y[0] > 129);
    assert!(pixels.y[4] > 129);
}
