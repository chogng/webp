//! VP8 scalar inverse transforms.

#[cfg(test)]
#[path = "transform_tests.rs"]
mod tests;

/// Performs VP8's integer inverse 4×4 DCT and returns pixel-domain residues.
///
/// Coefficients are in raster order after dequantization. All intermediates
/// use `i32`, preserving the specification's fixed-point rounding before the
/// final divide by eight.
#[must_use]
pub fn inverse_dct_4x4(coefficients: [i16; 16]) -> [i32; 16] {
    inverse_dct_4x4_i32(coefficients.map(i32::from))
}

/// Performs VP8's integer inverse 4×4 DCT on widened coefficients.
///
/// This is the reconstruction-facing form of [`inverse_dct_4x4`]. It keeps
/// dequantized coefficients in `i32`, so a malformed stream cannot force an
/// intermediate narrowing conversion before prediction and sample clipping.
#[must_use]
pub fn inverse_dct_4x4_i32(coefficients: [i32; 16]) -> [i32; 16] {
    let mut temporary = [0_i32; 16];
    for column in 0..4 {
        let a = coefficients[column] + coefficients[8 + column];
        let b = coefficients[column] - coefficients[8 + column];
        let c = transform_mul2_i32(coefficients[4 + column])
            - transform_mul1_i32(coefficients[12 + column]);
        let d = transform_mul1_i32(coefficients[4 + column])
            + transform_mul2_i32(coefficients[12 + column]);
        temporary[column * 4] = a + d;
        temporary[column * 4 + 1] = b + c;
        temporary[column * 4 + 2] = b - c;
        temporary[column * 4 + 3] = a - d;
    }

    let mut output = [0_i32; 16];
    for row in 0..4 {
        let dc = temporary[row] + 4;
        let a = dc + temporary[8 + row];
        let b = dc - temporary[8 + row];
        let c = transform_mul2_i32(temporary[4 + row]) - transform_mul1_i32(temporary[12 + row]);
        let d = transform_mul1_i32(temporary[4 + row]) + transform_mul2_i32(temporary[12 + row]);
        output[row * 4] = (a + d) >> 3;
        output[row * 4 + 1] = (b + c) >> 3;
        output[row * 4 + 2] = (b - c) >> 3;
        output[row * 4 + 3] = (a - d) >> 3;
    }
    output
}

/// Performs the VP8 4×4 inverse Walsh-Hadamard transform for Y2 DC values.
#[must_use]
pub fn inverse_wht_4x4(coefficients: [i16; 16]) -> [i32; 16] {
    inverse_wht_4x4_i32(coefficients.map(i32::from))
}

/// Performs VP8's integer inverse Walsh-Hadamard transform on widened Y2 DC
/// coefficients.
#[must_use]
pub fn inverse_wht_4x4_i32(coefficients: [i32; 16]) -> [i32; 16] {
    let mut temporary = [0_i32; 16];
    for column in 0..4 {
        let a0 = coefficients[column] + coefficients[12 + column];
        let a1 = coefficients[4 + column] + coefficients[8 + column];
        let a2 = coefficients[4 + column] - coefficients[8 + column];
        let a3 = coefficients[column] - coefficients[12 + column];
        temporary[column] = a0 + a1;
        temporary[8 + column] = a0 - a1;
        temporary[4 + column] = a3 + a2;
        temporary[12 + column] = a3 - a2;
    }

    let mut output = [0_i32; 16];
    for row in 0..4 {
        let dc = temporary[row * 4] + 3;
        let a0 = dc + temporary[3 + row * 4];
        let a1 = temporary[1 + row * 4] + temporary[2 + row * 4];
        let a2 = temporary[1 + row * 4] - temporary[2 + row * 4];
        let a3 = dc - temporary[3 + row * 4];
        output[row * 4] = (a0 + a1) >> 3;
        output[row * 4 + 1] = (a3 + a2) >> 3;
        output[row * 4 + 2] = (a0 - a1) >> 3;
        output[row * 4 + 3] = (a3 - a2) >> 3;
    }
    output
}

fn transform_mul1_i32(value: i32) -> i32 {
    let result = ((i64::from(value) * 20_091) >> 16) + i64::from(value);
    result.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}

fn transform_mul2_i32(value: i32) -> i32 {
    let result = (i64::from(value) * 35_468) >> 16;
    result.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}
