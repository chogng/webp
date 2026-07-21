use super::*;
use crate::coefficients::{CATEGORY_PROBABILITIES, COEFFICIENT_ZIGZAG};
use crate::test_support::{TestBoolWriter, coefficient_nodes, write_coefficient_eob};
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
        decode_intra_residuals(&mut decoder, &probabilities, false, &mut top, &mut left).unwrap();
    assert_eq!(residuals.y2.unwrap().end, 0);
    assert!(residuals.luma.iter().all(|block| block.non_zero == 0));
    assert!(residuals.u.iter().all(|block| block.non_zero == 0));
    assert!(residuals.v.iter().all(|block| block.non_zero == 0));
    assert_eq!(residuals.non_zero_y, 0);
    assert_eq!(residuals.non_zero_uv, 0);
    assert_eq!(top, ResidualContext::default());
    assert_eq!(left, ResidualContext::default());
}
