#![no_main]
#![forbid(unsafe_code)]

use libfuzzer_sys::fuzz_target;
use webp::DecodeLimits;
use webp_vp8::{parse_partition_layout, parse_riff_payload};

const MAX_VP8_BYTES: usize = 64 * 1024;

fn limits() -> DecodeLimits {
    DecodeLimits {
        max_input_bytes: MAX_VP8_BYTES,
        max_width: 256,
        max_height: 256,
        max_pixels: 256 * 256,
        max_work_units: 1_000_000,
        ..DecodeLimits::default()
    }
}

fuzz_target!(|payload: &[u8]| {
    if payload.len() > MAX_VP8_BYTES {
        return;
    }
    let limits = limits();
    if let Ok(frame) = parse_riff_payload(payload, None, &limits) {
        let _ = parse_partition_layout(payload, &frame, &limits);
    }
});
