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
