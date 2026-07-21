#![no_main]
#![forbid(unsafe_code)]

use libfuzzer_sys::fuzz_target;
use webp::{read_metadata, DecodeLimits};

fuzz_target!(|bytes: &[u8]| {
    let limits = DecodeLimits {
        max_input_bytes: 1 << 20,
        max_width: 4096,
        max_height: 4096,
        max_pixels: 16 * 1024 * 1024,
        max_metadata_bytes: 1 << 20,
        max_work_units: 10_000_000,
        ..DecodeLimits::default()
    };
    let _ = read_metadata(bytes, &limits);
});

