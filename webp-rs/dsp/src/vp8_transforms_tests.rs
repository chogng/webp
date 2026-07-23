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

#[test]
fn widened_entry_points_are_total_for_integer_extremes() {
    for values in [
        [i32::MIN; 16],
        [i32::MAX; 16],
        std::array::from_fn(|index| if index % 2 == 0 { i32::MIN } else { i32::MAX }),
    ] {
        let _ = forward_dct_4x4_i32(values);
        let _ = inverse_dct_4x4_i32(values);
        let _ = forward_wht_4x4_i32(values);
        let _ = inverse_wht_4x4_i32(values);
    }

    let alternating_i16 =
        std::array::from_fn(|index| if index % 2 == 0 { i16::MIN } else { i16::MAX });
    let _ = forward_dct_4x4(alternating_i16);
}

#[test]
fn forward_dct_matches_independent_scalar_reference_across_valid_residues() {
    let mut state = 0x6d2b_79f5_u32;
    for _ in 0..1_024 {
        let residues = std::array::from_fn(|_| {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            (state % 511) as i32 - 255
        });
        assert_eq!(
            forward_dct_4x4_i32(residues),
            reference_forward_dct(residues)
        );
    }
}

fn reference_forward_dct(residues: [i32; 16]) -> [i32; 16] {
    let mut temporary = [0_i64; 16];
    for (row, samples) in residues.chunks_exact(4).enumerate() {
        let d = [
            i64::from(samples[0]),
            i64::from(samples[1]),
            i64::from(samples[2]),
            i64::from(samples[3]),
        ];
        let sums = [d[0] + d[3], d[1] + d[2]];
        let differences = [d[1] - d[2], d[0] - d[3]];
        let offset = row * 4;
        temporary[offset] = (sums[0] + sums[1]) * 8;
        temporary[offset + 1] = (differences[0] * 2_217 + differences[1] * 5_352 + 1_812) >> 9;
        temporary[offset + 2] = (sums[0] - sums[1]) * 8;
        temporary[offset + 3] = (differences[1] * 2_217 - differences[0] * 5_352 + 937) >> 9;
    }

    let mut output = [0_i32; 16];
    for column in 0..4 {
        let even = [
            temporary[column] + temporary[12 + column],
            temporary[4 + column] + temporary[8 + column],
        ];
        let odd = [
            temporary[4 + column] - temporary[8 + column],
            temporary[column] - temporary[12 + column],
        ];
        output[column] = ((even[0] + even[1] + 7) >> 4) as i32;
        output[4 + column] =
            (((odd[0] * 2_217 + odd[1] * 5_352 + 12_000) >> 16) + i64::from(odd[1] != 0)) as i32;
        output[8 + column] = ((even[0] - even[1] + 7) >> 4) as i32;
        output[12 + column] = ((odd[1] * 2_217 - odd[0] * 5_352 + 51_000) >> 16) as i32;
    }
    output
}
