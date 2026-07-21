#![no_main]
#![forbid(unsafe_code)]

use libfuzzer_sys::fuzz_target;
use webp::{DecodeLimits, DecodeOptions, decode_animation};

fuzz_target!(|bytes: &[u8]| {
    let limits = DecodeLimits {
        max_input_bytes: 1 << 20,
        max_width: 4096,
        max_height: 4096,
        max_pixels: 16 * 1024 * 1024,
        max_frames: 512,
        max_total_frame_pixels: 16 * 1024 * 1024,
        max_alloc_bytes: 64 * 1024 * 1024,
        max_metadata_bytes: 1 << 20,
        max_work_units: 10_000_000,
    };
    let _ = decode_animation(
        bytes,
        &DecodeOptions {
            limits,
            ..DecodeOptions::default()
        },
    );
});
