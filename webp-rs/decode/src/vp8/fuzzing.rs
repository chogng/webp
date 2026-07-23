//! Bounded byte adapters for VP8 fuzz targets.

use super::*;
use crate::DecodeLimits;

const MAX_PARTITION_BYTES: usize = 64 * 1024;

fn limits(work: u64) -> DecodeLimits {
    DecodeLimits {
        max_input_bytes: MAX_PARTITION_BYTES,
        max_width: 256,
        max_height: 256,
        max_pixels: 256 * 256,
        max_work_units: work,
        ..DecodeLimits::default()
    }
}

pub(crate) fn bool_coder(input: &[u8]) {
    let split = input.len() / 2;
    let (payload, probabilities) = input.split_at(split);
    if payload.is_empty() || probabilities.is_empty() || payload.len() > MAX_PARTITION_BYTES {
        return;
    }
    let mut decoder = BoolDecoder::new(payload, &limits(4_096)).expect("bounded partition");
    for probability in probabilities.iter().copied().take(4_096) {
        if decoder.read_bool(probability).is_err() {
            break;
        }
    }
}

pub(crate) fn coefficients(input: &[u8]) {
    let Some((&selector, remainder)) = input.split_first() else {
        return;
    };
    let Some((&context, payload)) = remainder.split_first() else {
        return;
    };
    if payload.is_empty() || payload.len() > MAX_PARTITION_BYTES {
        return;
    }
    let coefficient_type = match selector & 3 {
        0 => CoefficientBlockType::Luma16Ac,
        1 => CoefficientBlockType::LumaDc,
        2 => CoefficientBlockType::ChromaAc,
        _ => CoefficientBlockType::Luma4Ac,
    };
    let mut decoder = BoolDecoder::new(payload, &limits(16 * 32)).expect("bounded partition");
    let _ = decode_coefficients(
        &mut decoder,
        &CoefficientProbabilities::default(),
        coefficient_type,
        context % 3,
        (selector >> 2) & 15,
    );
}

pub(crate) fn partition(payload: &[u8]) {
    if payload.len() > MAX_PARTITION_BYTES {
        return;
    }
    let limits = limits(1_000_000);
    if let Ok(frame) = parse_riff_payload(payload, None, &limits) {
        let _ = parse_partition_layout(payload, &frame, &limits);
        let _ = decode_intra_frame(payload, &frame, &limits);
    }
}

pub(crate) fn residuals(input: &[u8]) {
    let [selector, top_non_zero, left_non_zero, payload @ ..] = input else {
        return;
    };
    if payload.is_empty() || payload.len() > MAX_PARTITION_BYTES {
        return;
    }
    let mut decoder = BoolDecoder::new(payload, &limits(8_192)).expect("bounded partition");
    let mut top = ResidualContext {
        non_zero: *top_non_zero,
        non_zero_dc: selector & 2 != 0,
    };
    let mut left = ResidualContext {
        non_zero: *left_non_zero,
        non_zero_dc: selector & 4 != 0,
    };
    let _ = decode_intra_residuals(
        &mut decoder,
        &CoefficientProbabilities::default(),
        selector & 1 != 0,
        &mut top,
        &mut left,
    );
}

pub(crate) fn transforms(input: &[u8]) {
    let mut coefficients = [0_i16; 16];
    for (index, coefficient) in coefficients.iter_mut().enumerate() {
        *coefficient = i16::from_le_bytes([
            input.get(index * 2).copied().unwrap_or(0),
            input.get(index * 2 + 1).copied().unwrap_or(0),
        ]);
    }
    let _ = inverse_dct_4x4(coefficients);
    let _ = inverse_wht_4x4(coefficients);
    let _ = forward_dct_4x4(coefficients);
    let _ = forward_wht_4x4(coefficients);
    let widened = coefficients.map(i32::from);
    let residue = inverse_dct_4x4_i32(widened);
    let _ = inverse_wht_4x4_i32(widened);
    let _ = forward_dct_4x4_i32(widened);
    let _ = forward_wht_4x4_i32(widened);
    let arbitrary_i32 = std::array::from_fn(|index| {
        let offset = (index * 4) % 32;
        i32::from_le_bytes(std::array::from_fn(|byte| {
            input.get(offset + byte).copied().unwrap_or(0)
        }))
    });
    let _ = inverse_dct_4x4_i32(arbitrary_i32);
    let _ = inverse_wht_4x4_i32(arbitrary_i32);
    let _ = forward_dct_4x4_i32(arbitrary_i32);
    let _ = forward_wht_4x4_i32(arbitrary_i32);
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
}
