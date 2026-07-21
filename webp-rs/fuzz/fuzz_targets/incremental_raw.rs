#![no_main]
#![forbid(unsafe_code)]

use libfuzzer_sys::fuzz_target;
use webp::DecodeLimits;
use webp::DecodeOptions;
use webp::IncrementalDecoder;

fn limits() -> DecodeLimits {
    DecodeLimits {
        max_input_bytes: 1 << 20,
        max_width: 4096,
        max_height: 4096,
        max_pixels: 16 * 1024 * 1024,
        max_metadata_bytes: 1 << 20,
        max_work_units: 10_000_000,
        ..DecodeLimits::default()
    }
}

fuzz_target!(|input: &[u8]| {
    let mut decoder = IncrementalDecoder::new(DecodeOptions {
        limits: limits(),
        ..DecodeOptions::default()
    });
    let mut offset = 0;
    let mut chunk_seed = input.first().copied().unwrap_or(0);
    while offset < input.len() {
        let length = usize::from(chunk_seed & 0x1f).saturating_add(1);
        let end = offset.saturating_add(length).min(input.len());
        if decoder.push(&input[offset..end]).is_err() {
            return;
        }
        chunk_seed = chunk_seed.rotate_left(1).wrapping_add(0x3d);
        offset = end;
    }
    let _ = decoder.finish();
});
