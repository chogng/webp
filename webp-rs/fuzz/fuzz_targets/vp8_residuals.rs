#![no_main]
#![forbid(unsafe_code)]

use libfuzzer_sys::fuzz_target;
use webp::DecodeLimits;
use webp_vp8::BoolDecoder;
use webp_vp8::CoefficientProbabilities;
use webp_vp8::ResidualContext;
use webp_vp8::decode_intra_residuals;

const MAX_PARTITION_BYTES: usize = 64 * 1024;

fn limits() -> DecodeLimits {
    DecodeLimits {
        max_input_bytes: MAX_PARTITION_BYTES,
        max_work_units: 8_192,
        ..DecodeLimits::default()
    }
}

fuzz_target!(|input: &[u8]| {
    let [selector, top_non_zero, left_non_zero, payload @ ..] = input else {
        return;
    };
    if payload.is_empty() || payload.len() > MAX_PARTITION_BYTES {
        return;
    }
    let mut decoder = BoolDecoder::new(payload, &limits()).expect("bounded partition is valid");
    let selector = *selector;
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
});
