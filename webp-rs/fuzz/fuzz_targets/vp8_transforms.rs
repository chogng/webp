#![no_main]
#![forbid(unsafe_code)]

use libfuzzer_sys::fuzz_target;
use webp_vp8::Intra4Mode;
use webp_vp8::MacroblockPixels;
use webp_vp8::MacroblockSpatialResidues;
use webp_vp8::combine_macroblock_prediction;
use webp_vp8::inverse_dct_4x4;
use webp_vp8::inverse_dct_4x4_i32;
use webp_vp8::inverse_wht_4x4;
use webp_vp8::inverse_wht_4x4_i32;
use webp_vp8::predict_intra4_block;

fuzz_target!(|input: &[u8]| {
    let mut coefficients = [0_i16; 16];
    for (index, coefficient) in coefficients.iter_mut().enumerate() {
        let low = input.get(index * 2).copied().unwrap_or(0);
        let high = input.get(index * 2 + 1).copied().unwrap_or(0);
        *coefficient = i16::from_le_bytes([low, high]);
    }
    let _ = inverse_dct_4x4(coefficients);
    let _ = inverse_wht_4x4(coefficients);
    let widened = coefficients.map(i32::from);
    let residue = inverse_dct_4x4_i32(widened);
    let _ = inverse_wht_4x4_i32(widened);
    let _ = combine_macroblock_prediction(
        MacroblockPixels {
            y: [128; 256],
            u: [128; 64],
            v: [128; 64],
        },
        MacroblockSpatialResidues {
            luma: [residue; 16],
            u: [residue; 4],
            v: [residue; 4],
        },
    );
    let top = std::array::from_fn(|index| input.get(index).copied().unwrap_or(128));
    let left = std::array::from_fn(|index| input.get(8 + index).copied().unwrap_or(128));
    for mode in [
        Intra4Mode::Dc,
        Intra4Mode::TrueMotion,
        Intra4Mode::Vertical,
        Intra4Mode::Horizontal,
        Intra4Mode::DiagonalDownRight,
        Intra4Mode::VerticalRight,
        Intra4Mode::DiagonalDownLeft,
        Intra4Mode::VerticalLeft,
        Intra4Mode::HorizontalDown,
        Intra4Mode::HorizontalUp,
    ] {
        let _ = predict_intra4_block(mode, input.get(12).copied().unwrap_or(128), top, left);
    }
});
