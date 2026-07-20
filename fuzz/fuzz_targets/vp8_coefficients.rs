#![no_main]
#![forbid(unsafe_code)]

use libfuzzer_sys::fuzz_target;
use webp::DecodeLimits;
use webp_vp8::{BoolDecoder, CoefficientBlockType, CoefficientProbabilities, decode_coefficients};

const MAX_PARTITION_BYTES: usize = 64 * 1024;

fn limits() -> DecodeLimits {
    DecodeLimits {
        max_input_bytes: MAX_PARTITION_BYTES,
        max_work_units: 16 * 32,
        ..DecodeLimits::default()
    }
}

fuzz_target!(|input: &[u8]| {
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
    let start = (selector >> 2) & 15;
    let mut decoder = BoolDecoder::new(payload, &limits()).expect("bounded partition is valid");
    let _ = decode_coefficients(
        &mut decoder,
        &CoefficientProbabilities::default(),
        coefficient_type,
        context % 3,
        start,
    );
});
