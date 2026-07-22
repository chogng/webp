//! Fixed-width kernels for SharpYUV refinement.

use bytemuck::cast_slice;
use bytemuck::cast_slice_mut;
use wide::i16x8;

const LANES: usize = 8;
const WORKING_MAX: i16 = 1023;
const ROUND_VECTOR: i16x8 = i16x8::new([8; LANES]);
const ZERO_VECTOR: i16x8 = i16x8::new([0; LANES]);
const WORKING_MAX_VECTOR: i16x8 = i16x8::new([WORKING_MAX; LANES]);

pub(super) fn update_y(target: &[u16], reconstructed: &[u16], best: &mut [u16]) -> u64 {
    assert_eq!(target.len(), reconstructed.len());
    assert_eq!(target.len(), best.len());
    let mut difference = 0_u64;
    let vector_len = target.len() / LANES * LANES;
    for index in (0..vector_len).step_by(LANES) {
        let target = load_u16_as_i16(&target[index..]);
        let reconstructed = load_u16_as_i16(&reconstructed[index..]);
        let current = load_u16_as_i16(&best[index..]);
        let delta = target - reconstructed;
        let updated = (current + delta).max(ZERO_VECTOR).min(WORKING_MAX_VECTOR);
        store_i16_as_u16(updated, &mut best[index..]);
        difference += delta.abs().reduce_add() as u64;
    }
    difference
        + update_y_scalar(
            &target[vector_len..],
            &reconstructed[vector_len..],
            &mut best[vector_len..],
        )
}

pub(super) fn update_rgb(target: &[i16], reconstructed: &[i16], best: &mut [i16]) {
    assert_eq!(target.len(), reconstructed.len());
    assert_eq!(target.len(), best.len());
    let vector_len = target.len() / LANES * LANES;
    for index in (0..vector_len).step_by(LANES) {
        let target = i16x8::from_slice_unaligned(&target[index..]);
        let reconstructed = i16x8::from_slice_unaligned(&reconstructed[index..]);
        let current = i16x8::from_slice_unaligned(&best[index..]);
        best[index..index + LANES]
            .copy_from_slice((current + target - reconstructed).as_array_ref());
    }
    update_rgb_scalar(
        &target[vector_len..],
        &reconstructed[vector_len..],
        &mut best[vector_len..],
    );
}

/// Reconstructs two full-resolution samples for each adjacent chroma pair.
pub(super) fn filter_row(a: &[i16], b: &[i16], best_y: &[u16], output: &mut [u16]) {
    let len = a.len().saturating_sub(1);
    assert!(b.len() > len);
    assert_eq!(best_y.len(), 2 * len);
    assert_eq!(output.len(), 2 * len);
    let vector_len = len / LANES * LANES;
    for index in (0..vector_len).step_by(LANES) {
        let a0 = i16x8::from_slice_unaligned(&a[index..]);
        let a1 = i16x8::from_slice_unaligned(&a[index + 1..]);
        let b0 = i16x8::from_slice_unaligned(&b[index..]);
        let b1 = i16x8::from_slice_unaligned(&b[index + 1..]);
        let a0b1 = a0 + b1;
        let a1b0 = a1 + b0;
        let all = a0b1 + a1b0 + ROUND_VECTOR;
        let c0 = (a0b1 + a0b1 + all) >> 3_i32;
        let c1 = (a1b0 + a1b0 + all) >> 3_i32;
        let even = (c1 + a0) >> 1_i32;
        let odd = (c0 + a1) >> 1_i32;
        let even = even.as_array_ref();
        let odd = odd.as_array_ref();
        let first = i16x8::new([
            even[0], odd[0], even[1], odd[1], even[2], odd[2], even[3], odd[3],
        ]);
        let second = i16x8::new([
            even[4], odd[4], even[5], odd[5], even[6], odd[6], even[7], odd[7],
        ]);
        let output_start = 2 * index;
        let best_first = load_u16_as_i16(&best_y[output_start..]);
        let best_second = load_u16_as_i16(&best_y[output_start + LANES..]);
        store_i16_as_u16(
            (best_first + first)
                .max(ZERO_VECTOR)
                .min(WORKING_MAX_VECTOR),
            &mut output[output_start..],
        );
        store_i16_as_u16(
            (best_second + second)
                .max(ZERO_VECTOR)
                .min(WORKING_MAX_VECTOR),
            &mut output[output_start + LANES..],
        );
    }
    filter_row_scalar(
        &a[vector_len..],
        &b[vector_len..],
        &best_y[2 * vector_len..],
        &mut output[2 * vector_len..],
    );
}

fn update_y_scalar(target: &[u16], reconstructed: &[u16], best: &mut [u16]) -> u64 {
    target
        .iter()
        .zip(reconstructed)
        .zip(best)
        .map(|((&target, &reconstructed), best)| {
            let difference = i32::from(target) - i32::from(reconstructed);
            *best = (i32::from(*best) + difference).clamp(0, i32::from(WORKING_MAX)) as u16;
            difference.unsigned_abs() as u64
        })
        .sum()
}

fn update_rgb_scalar(target: &[i16], reconstructed: &[i16], best: &mut [i16]) {
    for ((&target, &reconstructed), best) in target.iter().zip(reconstructed).zip(best) {
        *best += target - reconstructed;
    }
}

fn filter_row_scalar(a: &[i16], b: &[i16], best_y: &[u16], output: &mut [u16]) {
    let len = a.len().saturating_sub(1);
    assert!(b.len() > len);
    assert_eq!(best_y.len(), 2 * len);
    assert_eq!(output.len(), 2 * len);
    for index in 0..len {
        let a0 = i32::from(a[index]);
        let a1 = i32::from(a[index + 1]);
        let b0 = i32::from(b[index]);
        let b1 = i32::from(b[index + 1]);
        let first = (a0 * 9 + a1 * 3 + b0 * 3 + b1 + 8) >> 4;
        let second = (a1 * 9 + a0 * 3 + b1 * 3 + b0 + 8) >> 4;
        output[2 * index] =
            (i32::from(best_y[2 * index]) + first).clamp(0, i32::from(WORKING_MAX)) as u16;
        output[2 * index + 1] =
            (i32::from(best_y[2 * index + 1]) + second).clamp(0, i32::from(WORKING_MAX)) as u16;
    }
}

#[inline(always)]
fn load_u16_as_i16(input: &[u16]) -> i16x8 {
    i16x8::from_slice_unaligned(cast_slice(&input[..LANES]))
}

#[inline(always)]
fn store_i16_as_u16(value: i16x8, output: &mut [u16]) {
    cast_slice_mut(&mut output[..LANES]).copy_from_slice(value.as_array_ref());
}

#[cfg(test)]
#[path = "kernels_tests.rs"]
mod tests;
