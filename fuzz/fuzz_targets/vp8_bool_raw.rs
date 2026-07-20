#![no_main]
#![forbid(unsafe_code)]

use libfuzzer_sys::fuzz_target;
use webp::DecodeLimits;
use webp_vp8::BoolDecoder;

const MAX_PARTITION_BYTES: usize = 64 * 1024;
const MAX_SYMBOLS: usize = 4_096;

fn limits() -> DecodeLimits {
    DecodeLimits {
        max_input_bytes: MAX_PARTITION_BYTES,
        max_work_units: MAX_SYMBOLS as u64,
        ..DecodeLimits::default()
    }
}

fuzz_target!(|input: &[u8]| {
    let split = input.len() / 2;
    let (payload, probabilities) = input.split_at(split);
    if payload.is_empty() || probabilities.is_empty() || payload.len() > MAX_PARTITION_BYTES {
        return;
    }
    let mut decoder = BoolDecoder::new(payload, &limits()).expect("bounded partition is valid");
    for probability in probabilities.iter().copied().take(MAX_SYMBOLS) {
        if decoder.read_bool(probability).is_err() {
            break;
        }
    }
});
