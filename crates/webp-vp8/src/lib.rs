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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coefficients::CATEGORY_PROBABILITIES;
    use crate::test_support::{
        TestBoolWriter, coefficient_nodes, key_frame, write_coefficient_eob,
        write_coefficient_updates, write_quantization_header,
    };
    use webp_core::{DecodeErrorKind, DecodeLimits};

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

    #[test]
    fn coefficient_decoder_handles_eob_zero_runs_signs_and_zigzag() {
        let probabilities = CoefficientProbabilities::default();
        let mut writer = TestBoolWriter::new();

        let initial = coefficient_nodes(&probabilities, CoefficientBlockType::Luma16Ac, 0, 0);
        writer.write_bool(true, initial[0]); // not EOB
        writer.write_bool(false, initial[1]); // zero at position zero
        let position_one = coefficient_nodes(&probabilities, CoefficientBlockType::Luma16Ac, 1, 0);
        writer.write_bool(false, position_one[1]); // zero at position one
        let position_two = coefficient_nodes(&probabilities, CoefficientBlockType::Luma16Ac, 2, 0);
        writer.write_bool(true, position_two[1]);
        writer.write_bool(false, position_two[2]); // magnitude one
        writer.write_bool(true, 128); // negative
        let next = coefficient_nodes(&probabilities, CoefficientBlockType::Luma16Ac, 3, 1);
        writer.write_bool(false, next[0]); // EOB

        let bytes = writer.finish();
        let mut decoder = BoolDecoder::new(&bytes, &DecodeLimits::default()).unwrap();
        let decoded = decode_coefficients(
            &mut decoder,
            &probabilities,
            CoefficientBlockType::Luma16Ac,
            0,
            0,
        )
        .unwrap();
        let mut expected = [0_i16; 16];
        expected[COEFFICIENT_ZIGZAG[2]] = -1;
        assert_eq!(decoded.values, expected);
        assert_eq!(decoded.end, 3);
        assert_eq!(decoded.non_zero, 1);
    }

    #[test]
    fn coefficient_decoder_handles_large_category_values_and_ac_only_start() {
        let probabilities = CoefficientProbabilities::default();
        let mut writer = TestBoolWriter::new();

        let nodes = coefficient_nodes(&probabilities, CoefficientBlockType::Luma4Ac, 1, 2);
        writer.write_bool(true, nodes[0]); // not EOB
        writer.write_bool(true, nodes[1]); // non-zero
        writer.write_bool(true, nodes[2]); // value exceeds one
        writer.write_bool(true, nodes[3]); // category path
        writer.write_bool(true, nodes[6]);
        writer.write_bool(false, nodes[8]);
        writer.write_bool(true, nodes[9]);
        for &probability in CATEGORY_PROBABILITIES[2] {
            writer.write_bool(false, probability);
        }
        writer.write_bool(false, 128); // positive sign
        let next = coefficient_nodes(&probabilities, CoefficientBlockType::Luma4Ac, 2, 2);
        writer.write_bool(false, next[0]); // EOB

        let bytes = writer.finish();
        let mut decoder = BoolDecoder::new(&bytes, &DecodeLimits::default()).unwrap();
        let decoded = decode_coefficients(
            &mut decoder,
            &probabilities,
            CoefficientBlockType::Luma4Ac,
            2,
            1,
        )
        .unwrap();
        let mut expected = [0_i16; 16];
        // This uses the category-five branch (base magnitude 35) with a zero
        // suffix, exercising the longest category tree selected by this
        // compact vector.
        expected[COEFFICIENT_ZIGZAG[1]] = 35;
        assert_eq!(decoded.values, expected);
        assert_eq!(decoded.end, 2);
        assert_eq!(decoded.non_zero, 1);
    }

    #[test]
    fn coefficient_decoder_rejects_invalid_context_and_start() {
        let probabilities = CoefficientProbabilities::default();
        let mut decoder = BoolDecoder::new(&[0], &DecodeLimits::default()).unwrap();
        assert_eq!(
            decode_coefficients(
                &mut decoder,
                &probabilities,
                CoefficientBlockType::LumaDc,
                3,
                0,
            )
            .unwrap_err()
            .kind(),
            DecodeErrorKind::InvalidParameter
        );
        assert_eq!(
            decode_coefficients(
                &mut decoder,
                &probabilities,
                CoefficientBlockType::LumaDc,
                0,
                16,
            )
            .unwrap_err()
            .kind(),
            DecodeErrorKind::InvalidParameter
        );
    }

    #[test]
    fn intra_frame_decoder_reconstructs_a_skipped_macroblock() {
        let mut writer = TestBoolWriter::new();
        writer.write_bool(false, 128); // colour space
        writer.write_bool(false, 128); // clamp type
        writer.write_bool(false, 128); // no segmentation
        writer.write_bool(false, 128); // normal filter
        writer.write_literal(0, 6); // filter level
        writer.write_literal(0, 3); // filter sharpness
        writer.write_bool(false, 128); // no filter deltas
        writer.write_literal(0, 2); // one token partition
        write_quantization_header(&mut writer, 0, [0; 5], false);
        write_coefficient_updates(&mut writer, &[], true, 1);
        writer.write_bool(true, 1); // skip residuals
        writer.write_bool(true, 145); // 16x16 luma
        writer.write_bool(false, 156); // DC luma
        writer.write_bool(false, 163);
        writer.write_bool(false, 142); // DC chroma
        let partition_zero = writer.finish();
        let mut payload = key_frame(1, 1, 0, true, partition_zero.len() as u32).to_vec();
        payload.extend_from_slice(&partition_zero);
        payload.push(0); // Non-empty final token partition, never consumed by skip.

        let limits = DecodeLimits::default();
        let frame = parse_riff_payload(&payload, None, &limits).unwrap();
        let image = decode_intra_frame(&payload, &frame, &limits).unwrap();
        assert_eq!(image.width, 1);
        assert_eq!(image.height, 1);
        assert_eq!(image.y_stride, 16);
        assert_eq!(image.uv_stride, 8);
        assert_eq!(image.y.len(), 16 * 16);
        assert_eq!(image.u.len(), 8 * 8);
        assert!(image.y.iter().all(|&sample| sample == 128));
        assert!(image.u.iter().all(|&sample| sample == 128));
        assert!(image.v.iter().all(|&sample| sample == 128));
    }

    #[test]
    fn residual_decoder_consumes_all_intra_block_families_and_preserves_empty_contexts() {
        let probabilities = CoefficientProbabilities::default();
        let mut writer = TestBoolWriter::new();
        write_coefficient_eob(
            &mut writer,
            &probabilities,
            CoefficientBlockType::LumaDc,
            0,
            0,
        );
        for _ in 0..16 {
            write_coefficient_eob(
                &mut writer,
                &probabilities,
                CoefficientBlockType::Luma16Ac,
                1,
                0,
            );
        }
        for _ in 0..8 {
            write_coefficient_eob(
                &mut writer,
                &probabilities,
                CoefficientBlockType::ChromaAc,
                0,
                0,
            );
        }

        let bytes = writer.finish();
        let mut decoder = BoolDecoder::new(&bytes, &DecodeLimits::default()).unwrap();
        let mut top = ResidualContext::default();
        let mut left = ResidualContext::default();
        let residuals =
            decode_intra_residuals(&mut decoder, &probabilities, false, &mut top, &mut left)
                .unwrap();
        assert_eq!(residuals.y2.unwrap().end, 0);
        assert!(residuals.luma.iter().all(|block| block.non_zero == 0));
        assert!(residuals.u.iter().all(|block| block.non_zero == 0));
        assert!(residuals.v.iter().all(|block| block.non_zero == 0));
        assert_eq!(residuals.non_zero_y, 0);
        assert_eq!(residuals.non_zero_uv, 0);
        assert_eq!(top, ResidualContext::default());
        assert_eq!(left, ResidualContext::default());
    }
}
