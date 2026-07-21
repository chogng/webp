#![no_main]
#![forbid(unsafe_code)]

use libfuzzer_sys::fuzz_target;
use webp::DecodeLimits;
use webp::DecodeOptions;

const MAX_RAW_VP8L_BYTES: usize = 64 * 1024;
const RIFF_OVERHEAD: usize = 20;

fn limits() -> DecodeLimits {
    DecodeLimits {
        max_input_bytes: MAX_RAW_VP8L_BYTES + RIFF_OVERHEAD,
        max_width: 256,
        max_height: 256,
        max_pixels: 256 * 256,
        max_frames: 1,
        max_total_frame_pixels: 256 * 256,
        max_metadata_bytes: 0,
        max_alloc_bytes: 512 * 1024,
        max_work_units: 1_000_000,
        ..DecodeLimits::default()
    }
}

/// Presents an entropy-stream mutation as a complete WebP file so the fuzz
/// target exercises the public decoder rather than codec-private parsers.
fn wrap_vp8l(raw: &[u8]) -> Vec<u8> {
    let padding = raw.len() & 1;
    let riff_size = 12 + raw.len() + padding;
    let mut file = Vec::with_capacity(RIFF_OVERHEAD + raw.len() + padding);
    file.extend_from_slice(b"RIFF");
    file.extend_from_slice(&(riff_size as u32).to_le_bytes());
    file.extend_from_slice(b"WEBPVP8L");
    file.extend_from_slice(&(raw.len() as u32).to_le_bytes());
    file.extend_from_slice(raw);
    if padding != 0 {
        file.push(0);
    }
    file
}

fuzz_target!(|raw: &[u8]| {
    if raw.len() > MAX_RAW_VP8L_BYTES {
        return;
    }
    let file = wrap_vp8l(raw);
    let _ = webp::decode(
        &file,
        &DecodeOptions {
            limits: limits(),
            ..DecodeOptions::default()
        },
    );
});
