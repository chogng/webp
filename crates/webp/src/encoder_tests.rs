use super::*;
use crate::{DecodeOptions, decode};

#[test]
fn literal_vp8l_encoder_round_trips_straight_rgba() {
    let rgba = [
        0, 1, 2, 255, 17, 34, 51, 255, 255, 127, 63, 255, 1, 2, 3, 4, 0, 255, 32, 128, 91, 7, 203,
        0,
    ];
    let encoded = encode_lossless_rgba(3, 2, &rgba).expect("encode literal VP8L");
    assert_eq!(&encoded[..4], b"RIFF");
    assert_eq!(&encoded[12..16], b"VP8L");
    assert_eq!(
        decode(&encoded, &DecodeOptions::default())
            .expect("decode encoded VP8L")
            .rgba,
        rgba
    );
}

#[test]
fn predictor_mode_subimage_round_trips_across_block_edges() {
    let mut rgba = Vec::new();
    for index in 0..25_u8 {
        rgba.extend_from_slice(&[
            index.wrapping_mul(17),
            index.wrapping_mul(31),
            index.wrapping_mul(47),
            if index.is_multiple_of(3) { 255 } else { index },
        ]);
    }
    let encoded = encode_lossless_rgba(5, 5, &rgba).expect("encode predictor block-edge image");
    assert_eq!(
        decode(&encoded, &DecodeOptions::default())
            .expect("decode predictor block-edge image")
            .rgba,
        rgba
    );
}

#[test]
fn encoder_uses_color_indexing_for_repeated_small_palettes() {
    let rgba = [
        10, 20, 30, 255, 40, 50, 60, 128, 10, 20, 30, 255, 40, 50, 60, 128, 10, 20, 30, 255, 40,
        50, 60, 128, 10, 20, 30, 255, 40, 50, 60, 128,
    ];
    let encoded = encode_lossless_rgba(8, 1, &rgba).expect("encode indexed VP8L");
    // The fixed header is five bytes after the VP8L chunk header. The first
    // transform is present and has wire type 3 (color indexing).
    assert_eq!(encoded[25] & 0b111, 0b111);
    assert_eq!(
        decode(&encoded, &DecodeOptions::default())
            .expect("decode indexed VP8L")
            .rgba,
        rgba
    );
}

#[test]
fn encoder_selects_a_bounded_color_cache_only_when_it_hits() {
    assert_eq!(
        select_color_cache_bits(&[1, 2, 3, 4], 1, false, false),
        0,
        "a one-pixel direct stream has no cache hit"
    );
    assert!(
        select_color_cache_bits(&[1, 2, 3, 4, 1, 2, 3, 4], 2, false, false) >= 1,
        "a repeated direct pixel should enable a bounded cache"
    );
}

#[test]
fn encoder_emits_bounded_distance_one_lz77_runs() {
    let mut rgba = Vec::new();
    for _ in 0..16 {
        rgba.extend_from_slice(&[10, 20, 30, 255]);
    }
    let (tokens, _) =
        collect_entropy_tokens(&rgba, 16, true, true, 0).expect("tokenize repeated row");
    assert!(
        tokens
            .iter()
            .any(|token| matches!(token, EntropyToken::Copy { length } if *length >= 3)),
        "repeated residuals use a VP8L copy token"
    );
    let encoded = encode_lossless_rgba(16, 1, &rgba).expect("encode repeated row");
    assert_eq!(
        decode(&encoded, &DecodeOptions::default())
            .expect("decode repeated row")
            .rgba,
        rgba
    );
}

#[test]
fn encoder_selects_left_prediction_only_for_repeated_transformed_neighbours() {
    let repeated = [
        10, 20, 30, 255, 10, 20, 30, 255, 10, 20, 30, 255, 10, 20, 30, 255,
    ];
    assert!(select_left_predictor(&repeated, 4));
    let varied = [0, 0, 0, 255, 1, 3, 5, 255, 8, 13, 21, 255, 34, 55, 89, 255];
    assert!(!select_left_predictor(&varied, 4));
}

#[test]
fn encoder_selects_and_round_trips_a_strong_global_color_transform() {
    let mut rgba = Vec::new();
    for green in 0_u8..=u8::MAX {
        rgba.extend_from_slice(&[green.wrapping_add(3), green, green.wrapping_sub(5), 255]);
    }
    assert!(
        select_color_transform(&rgba).is_some(),
        "correlated RGB channels clear the transform's bounded selection threshold"
    );
    let encoded = encode_lossless_rgba(16, 16, &rgba).expect("encode color-transformed VP8L");
    assert_eq!(
        decode(&encoded, &DecodeOptions::default())
            .expect("decode color-transformed VP8L")
            .rgba,
        rgba
    );
}

#[test]
fn encoder_round_trips_static_vp8l_geometry_and_alpha_matrix() {
    let cases = [
        (1, 1),
        (1, 5),
        (5, 1),
        (3, 5),
        (5, 3),
        (4, 4),
        (5, 5),
        (16, 1),
        (1, 16),
    ];
    let mut state = 0x4d_34_c3_11_u32;
    for (width, height) in cases {
        let mut rgba = Vec::new();
        for _ in 0..width * height * 4 {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            rgba.push(state as u8);
        }
        let encoded =
            encode_lossless_rgba(width, height, &rgba).expect("encode static VP8L geometry case");
        let decoded =
            decode(&encoded, &DecodeOptions::default()).expect("decode static VP8L geometry case");
        assert_eq!((decoded.width, decoded.height), (width, height));
        assert_eq!(decoded.rgba, rgba, "{width} by {height}");
    }
}

#[test]
fn literal_vp8l_encoder_rejects_unrepresentable_dimensions_and_bad_input_length() {
    let pixel = [0_u8; 4];
    assert_eq!(
        encode_lossless_rgba(0, 1, &pixel).unwrap_err(),
        EncodeError::invalid_dimensions()
    );
    assert_eq!(
        encode_lossless_rgba(MAX_DIMENSION + 1, 1, &pixel).unwrap_err(),
        EncodeError::invalid_dimensions()
    );
    assert_eq!(
        encode_lossless_rgba(1, 1, &[]).unwrap_err(),
        EncodeError::invalid_rgba_length()
    );
}
