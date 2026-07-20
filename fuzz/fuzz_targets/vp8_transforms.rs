#![no_main]
#![forbid(unsafe_code)]

use libfuzzer_sys::fuzz_target;
use webp_vp8::{inverse_dct_4x4, inverse_wht_4x4};

fuzz_target!(|input: &[u8]| {
    let mut coefficients = [0_i16; 16];
    for (index, coefficient) in coefficients.iter_mut().enumerate() {
        let low = input.get(index * 2).copied().unwrap_or(0);
        let high = input.get(index * 2 + 1).copied().unwrap_or(0);
        *coefficient = i16::from_le_bytes([low, high]);
    }
    let _ = inverse_dct_4x4(coefficients);
    let _ = inverse_wht_4x4(coefficients);
});
