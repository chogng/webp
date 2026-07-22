//! Tests for VP8 transform kernels.

use super::*;

#[test]
fn inverse_dct_preserves_zero_and_dc_microvectors() {
    assert_eq!(inverse_dct_4x4([0; 16]), [0; 16]);
    let mut dc = [0_i16; 16];
    dc[0] = 16;
    assert_eq!(inverse_dct_4x4(dc), [2; 16]);
}

#[test]
fn inverse_wht_distributes_y2_dc_to_all_macroblock_blocks() {
    assert_eq!(inverse_wht_4x4([0; 16]), [0; 16]);
    let mut dc = [0_i16; 16];
    dc[0] = 8;
    assert_eq!(inverse_wht_4x4(dc), [1; 16]);
}

#[test]
fn forward_dct_matches_vp8_scale_for_constant_and_reconstructs_residues() {
    // VP8's fixed-point AC rounding leaves a deterministic coefficient at
    // position one even for this constant block.
    assert_eq!(
        forward_dct_4x4([3; 16]),
        [24, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
    );

    let samples = [-14, -5, 3, 12, -9, 2, 8, 15, -3, 1, 11, 7, -12, -2, 4, 13];
    let reconstructed = inverse_dct_4x4_i32(forward_dct_4x4_i32(samples));
    for (actual, expected) in reconstructed.into_iter().zip(samples) {
        assert!(
            (actual - expected).abs() <= 1,
            "{actual} differs from {expected}"
        );
    }
}

#[test]
fn forward_wht_round_trips_integer_dc_layout() {
    let values = [-9, 2, 6, 1, 4, -3, 8, -5, 7, 0, -2, 3, 1, -7, 5, 9];
    let reconstructed = inverse_wht_4x4_i32(forward_wht_4x4_i32(values));
    assert_eq!(reconstructed, values);
}
