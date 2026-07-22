use super::*;
use webp_decode::DecodeOptions;
use webp_decode::decode;

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
fn encoder_uses_cache_and_lz77_on_non_palette_images() {
    let (cache_width, cache_rgba) = non_palette_cache_input();
    assert!(
        try_make_palette_plan(&cache_rgba, cache_width)
            .expect("inspect cache input palette")
            .is_none(),
        "the cache input has more than the encoder's palette bound"
    );
    let cache_bits = select_color_cache_bits(&cache_rgba, cache_width, true, false);
    let (cache_tokens, _) =
        collect_entropy_tokens(&cache_rgba, cache_width, true, false, cache_bits)
            .expect("tokenize cache input");
    assert!(
        cache_tokens
            .iter()
            .any(|token| matches!(token, EntropyToken::Cache(_))),
        "a non-adjacent repeat reaches the main-stream color cache"
    );
    let cache_encoded =
        encode_lossless_rgba(cache_width as u32, 1, &cache_rgba).expect("encode cache input");
    assert_eq!(
        decode(&cache_encoded, &DecodeOptions::default())
            .expect("decode cache input")
            .rgba,
        cache_rgba
    );

    let (copy_width, copy_height, copy_rgba) = non_palette_copy_input();
    assert!(
        try_make_palette_plan(&copy_rgba, copy_width)
            .expect("inspect copy input palette")
            .is_none(),
        "the copy input has more than the encoder's palette bound"
    );
    assert!(select_left_predictor(&copy_rgba, copy_width));
    let (copy_tokens, _) =
        collect_entropy_tokens(&copy_rgba, copy_width, true, true, 0).expect("tokenize copy input");
    assert!(
        copy_tokens
            .iter()
            .any(|token| matches!(token, EntropyToken::Copy { length } if *length >= 3)),
        "repeated non-palette rows reach bounded distance-one LZ77"
    );
    let copy_encoded = encode_lossless_rgba(copy_width as u32, copy_height as u32, &copy_rgba)
        .expect("encode copy input");
    assert_eq!(
        decode(&copy_encoded, &DecodeOptions::default())
            .expect("decode copy input")
            .rgba,
        copy_rgba
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
fn color_transform_wire_size_matches_block_image_at_boundaries() {
    for (width, height) in [
        (127, 129),
        (128, 128),
        (129, 127),
        (129, 129),
        (511, 129),
        (512, 128),
        (513, 127),
    ] {
        let rgba = correlated_color_input(width, height, false);
        assert!(
            select_color_transform(&rgba).is_some(),
            "{width} by {height} must exercise the color transform"
        );
        let encoded = encode_lossless_rgba(width, height, &rgba)
            .expect("encode color-transform boundary case");
        assert_eq!(
            (encoded[25] >> 3) & 0b111,
            COLOR_TRANSFORM_BLOCK_BITS - 2,
            "wire size stores the block exponent minus VP8L's mandatory two"
        );
        assert_eq!(
            decode(&encoded, &DecodeOptions::default())
                .expect("decode color-transform boundary case")
                .rgba,
            rgba,
            "{width} by {height}"
        );
    }
}

#[test]
fn negative_color_transform_coefficients_round_trip() {
    let rgba = correlated_color_input(129, 129, true);
    let plan = select_color_transform(&rgba).expect("select negative color transform");
    assert!(
        plan.green_to_red < 0 || plan.green_to_blue < 0 || plan.red_to_blue < 0,
        "input must exercise a negative coefficient"
    );
    let encoded = encode_lossless_rgba(129, 129, &rgba).expect("encode negative transform");
    assert_eq!(
        decode(&encoded, &DecodeOptions::default())
            .expect("decode negative transform")
            .rgba,
        rgba
    );
}

#[test]
fn bounded_lossy_vp8_api_encodes_opaque_macroblocks_at_explicit_quality() {
    let mut rgba = Vec::new();
    for y in 0_u8..16 {
        for x in 0_u8..16 {
            rgba.extend_from_slice(&[
                x.wrapping_mul(15),
                y.wrapping_mul(15),
                x.wrapping_add(y).wrapping_mul(7),
                255,
            ]);
        }
    }
    for quality in [0, 75, 100] {
        let encoded = encode_lossy_rgba_with_options(16, 16, &rgba, LossyEncodeOptions { quality })
            .expect("encode bounded lossy VP8 profile");
        let decoded = decode(&encoded, &DecodeOptions::default()).expect("decode lossy VP8");
        assert_eq!((decoded.width, decoded.height), (16, 16));
        assert!(decoded.rgba.chunks_exact(4).all(|pixel| pixel[3] == 255));
    }
    assert_eq!(
        encode_lossy_rgba_with_options(16, 16, &rgba, LossyEncodeOptions { quality: 101 }),
        Err(EncodeError::invalid_quality())
    );
    let mut edge_rgba = Vec::new();
    for y in 0_u8..3 {
        for x in 0_u8..17 {
            edge_rgba.extend_from_slice(&[
                x.wrapping_mul(13),
                y.wrapping_mul(61),
                x.wrapping_add(y).wrapping_mul(11),
                255,
            ]);
        }
    }
    let edge = encode_lossy_rgba(17, 3, &edge_rgba).expect("encode visible-edge VP8 frame");
    let edge_decoded = decode(&edge, &DecodeOptions::default()).expect("decode visible-edge VP8");
    assert_eq!((edge_decoded.width, edge_decoded.height), (17, 3));

    let mut translucent = rgba;
    for (index, pixel) in translucent.chunks_exact_mut(4).enumerate() {
        pixel[3] = u8::try_from(index).expect("16 by 16 alpha index fits u8");
    }
    let alpha_encoded = encode_lossy_rgba(16, 16, &translucent).expect("encode VP8 with alpha");
    assert_eq!(&alpha_encoded[12..16], b"VP8X");
    let alpha_decoded =
        decode(&alpha_encoded, &DecodeOptions::default()).expect("decode VP8 with alpha");
    assert_eq!(
        alpha_decoded
            .rgba
            .chunks_exact(4)
            .map(|pixel| pixel[3])
            .collect::<Vec<_>>(),
        translucent
            .chunks_exact(4)
            .map(|pixel| pixel[3])
            .collect::<Vec<_>>()
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

fn non_palette_cache_input() -> (usize, Vec<u8>) {
    let mut rgba = Vec::new();
    for _ in 0..2 {
        for value in 0_u8..18 {
            rgba.extend_from_slice(&[value.wrapping_mul(13), 0, value.wrapping_mul(29), 255]);
        }
    }
    (36, rgba)
}

fn non_palette_copy_input() -> (usize, usize, Vec<u8>) {
    let width = 32;
    let mut rgba = Vec::new();
    for row in 0_u8..17 {
        let pixel = [
            row.wrapping_mul(11).wrapping_add(7),
            row.wrapping_mul(13).wrapping_add(19),
            row.wrapping_mul(17).wrapping_add(29),
            255,
        ];
        for _ in 0..width {
            rgba.extend_from_slice(&pixel);
        }
    }
    (width, 17, rgba)
}

fn correlated_color_input(width: u32, height: u32, negative: bool) -> Vec<u8> {
    let mut rgba = Vec::with_capacity((width * height * 4) as usize);
    for y in 0..height {
        for x in 0..width {
            let green = x.wrapping_add(y.wrapping_mul(3)) as u8;
            let (red, blue) = if negative {
                (green.wrapping_neg(), green.wrapping_neg().wrapping_add(7))
            } else {
                (green.wrapping_add(3), green.wrapping_sub(5))
            };
            rgba.extend_from_slice(&[red, green, blue, u8::MAX]);
        }
    }
    rgba
}
