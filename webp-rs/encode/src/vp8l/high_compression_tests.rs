use super::*;
use crate::{
    LosslessEncodeOptions, LosslessEncodeProfile, encode_lossless_rgba,
    encode_lossless_rgba_with_options,
};
use webp_decode::{DecodeOptions, decode};

fn tiled_image(width: usize, height: usize) -> Vec<u8> {
    let mut tile = Vec::new();
    for index in 0..32_u8 {
        tile.extend_from_slice(&[
            index.wrapping_mul(17),
            index.wrapping_mul(29),
            index.wrapping_mul(43),
            if index.is_multiple_of(5) { index } else { 255 },
        ]);
    }
    let mut rgba = Vec::with_capacity(width * height * 4);
    for index in 0..width * height {
        let pixel = index % 32;
        rgba.extend_from_slice(&tile[pixel * 4..pixel * 4 + 4]);
    }
    rgba
}

#[test]
fn high_compression_round_trips_and_beats_the_legacy_bounded_stream() {
    let width = 256;
    let height = 64;
    let rgba = tiled_image(width, height);
    let encoded = encode_lossless_rgba_with_options(
        width as u32,
        height as u32,
        &rgba,
        LosslessEncodeOptions {
            profile: LosslessEncodeProfile::HighCompression,
        },
    )
    .expect("encode bounded high-compression profile");
    let baseline =
        encode_lossless_rgba(width as u32, height as u32, &rgba).expect("encode default profile");
    assert!(
        encoded.len() < baseline.len(),
        "high-compression {} default {}",
        encoded.len(),
        baseline.len()
    );
    let decoded =
        decode(&encoded, &DecodeOptions::default()).expect("decode high-compression stream");
    assert_eq!(
        (decoded.width, decoded.height),
        (width as u32, height as u32)
    );
    assert_eq!(decoded.rgba, rgba);
}

#[test]
fn high_compression_preserves_tiny_palette_and_transparent_inputs() {
    for (width, height, rgba) in [
        (1, 1, vec![1, 2, 3, 0]),
        (16, 2, [10, 20, 30, 255, 40, 50, 60, 17].repeat(16)),
    ] {
        let encoded = encode(width, height, &rgba).expect("encode high-compression edge input");
        assert_eq!(
            decode(&encoded, &DecodeOptions::default())
                .expect("decode edge input")
                .rgba,
            rgba
        );
    }
}

#[test]
fn high_compression_round_trips_a_full_byte_palette() {
    let width = 256;
    let mut row = Vec::new();
    for index in 0..256_u16 {
        row.extend_from_slice(&[
            index as u8,
            index.wrapping_mul(29) as u8,
            index.wrapping_mul(71) as u8,
            u8::MAX,
        ]);
    }
    let rgba = row.repeat(4);
    let encoded = encode(width, 4, &rgba).expect("encode full byte palette");
    let decoded = decode(&encoded, &DecodeOptions::default()).expect("decode full byte palette");
    assert_eq!(decoded.rgba, rgba);
}
