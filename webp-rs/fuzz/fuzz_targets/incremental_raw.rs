#![no_main]
#![forbid(unsafe_code)]

use libfuzzer_sys::fuzz_target;
use webp::DecodeLimits;
use webp::DecodeOptions;
use webp::IncrementalDecoder;
use webp::decode;

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
    let options = DecodeOptions {
        limits: limits(),
        ..DecodeOptions::default()
    };
    let one_shot = decode(input, &options);
    let mut decoder = IncrementalDecoder::new(options);
    let mut offset = 0;
    let mut chunk_seed = input.first().copied().unwrap_or(0);
    let mut previous_rows = 0;
    let mut push_error = None;
    while offset < input.len() {
        let length = usize::from(chunk_seed & 0x1f).saturating_add(1);
        let end = offset.saturating_add(length).min(input.len());
        if let Err(error) = decoder.push(&input[offset..end]) {
            push_error = Some(error);
            break;
        }
        if let Some(image) = decoder.decoded() {
            assert!(image.decoded_rows >= previous_rows);
            assert!(image.decoded_rows <= image.height);
            assert_eq!(
                image.rgba.len(),
                image.width as usize * image.decoded_rows as usize * 4
            );
            previous_rows = image.decoded_rows;
        }
        chunk_seed = chunk_seed.rotate_left(1).wrapping_add(0x3d);
        offset = end;
    }
    let incremental = push_error.map_or_else(|| decoder.finish(), Err);
    match (one_shot, incremental) {
        (Ok(expected), Ok(actual)) => assert_eq!(actual, expected),
        (Ok(_), Err(error)) => panic!("incremental rejected one-shot input: {error}"),
        (Err(_), Ok(_)) => panic!("incremental accepted one-shot rejection"),
        (Err(_), Err(_)) => {}
    }
});
