use super::*;
use crate::DecodeLimits;
use crate::vp8::decode_intra_frame;
use crate::vp8::parse_riff_payload;

#[test]
fn neutral_key_frames_parse_and_decode_at_visible_edge_sizes() {
    for (width, height) in [(1, 1), (16, 16), (17, 3)] {
        let encoded =
            encode_neutral_key_frame(width, height).expect("encode neutral VP8 key frame");
        let header = parse_riff_payload(&encoded, None, &DecodeLimits::default())
            .expect("parse neutral VP8 key frame");
        assert_eq!((header.width, header.height), (width, height));
        let decoded = decode_intra_frame(&encoded, &header, &DecodeLimits::default())
            .expect("decode neutral VP8 key frame");
        assert_eq!((decoded.width, decoded.height), (width, height));
    }
}

#[test]
fn neutral_key_frame_rejects_unrepresentable_dimensions() {
    assert_eq!(
        encode_neutral_key_frame(0, 1),
        Err(Vp8EncodeError::InvalidDimensions)
    );
    assert_eq!(
        encode_neutral_key_frame(0x4000, 1),
        Err(Vp8EncodeError::InvalidDimensions)
    );
}

#[test]
fn rgba_to_yuv420_pads_visible_edges_to_whole_macroblocks() {
    let rgba = [255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255];
    let yuv = rgba_to_yuv420(3, 1, &rgba).expect("convert RGBA to VP8 YUV");
    assert_eq!((yuv.y_stride, yuv.uv_stride), (16, 8));
    assert_eq!(yuv.y.len(), 16 * 16);
    assert_eq!(yuv.u.len(), 8 * 8);
    assert_eq!(&yuv.y[..3], &[82, 144, 41]);
    assert_eq!(yuv.y[3], 41, "right edge is replicated");
    assert_eq!(yuv.y[16], 82, "bottom edge is replicated");
    assert_eq!(
        rgba_to_yuv420(1, 1, &[]),
        Err(Vp8EncodeError::InvalidRgbaLength)
    );
}

#[test]
fn dc_macroblock_quantization_routes_luma_dc_through_y2() {
    let matrix = crate::vp8::derive_dequantization(
        crate::vp8::QuantizationHeader {
            base_index: 0,
            y1_dc_delta: 0,
            y2_dc_delta: 0,
            y2_ac_delta: 0,
            uv_dc_delta: 0,
            uv_ac_delta: 0,
        },
        &crate::vp8::SegmentHeader {
            enabled: false,
            update_map: false,
            absolute_delta: false,
            quantizer: [0; 4],
            filter_strength: [0; 4],
            probabilities: [255; 3],
        },
    )[0];
    let coefficients = quantize_dc_macroblock(
        &[134; 16 * 16],
        16,
        &[128; 8 * 8],
        &[128; 8 * 8],
        8,
        [128; 3],
        matrix,
    )
    .unwrap();
    assert_eq!(coefficients.y2[0], 48);
    assert!(coefficients.y2[1..].iter().all(|&value| value == 0));
    assert!(coefficients.luma.iter().flatten().all(|&value| value == 0));
    assert!(coefficients.u.iter().flatten().all(|&value| value == 0));
    assert!(coefficients.v.iter().flatten().all(|&value| value == 0));
}

#[test]
fn dc_macroblock_quantization_rejects_short_planes() {
    let matrix = crate::vp8::derive_dequantization(
        crate::vp8::QuantizationHeader {
            base_index: 0,
            y1_dc_delta: 0,
            y2_dc_delta: 0,
            y2_ac_delta: 0,
            uv_dc_delta: 0,
            uv_ac_delta: 0,
        },
        &crate::vp8::SegmentHeader {
            enabled: false,
            update_map: false,
            absolute_delta: false,
            quantizer: [0; 4],
            filter_strength: [0; 4],
            probabilities: [255; 3],
        },
    )[0];
    assert_eq!(
        quantize_dc_macroblock(&[128; 15], 16, &[128; 64], &[128; 64], 8, [128; 3], matrix),
        Err(Vp8EncodeError::InvalidPlaneLayout)
    );
}

#[test]
fn dc_predicted_macroblock_key_frame_rejects_non_macroblock_geometry() {
    let source = Vp8SourceYuv {
        width: 1,
        height: 1,
        y_stride: 16,
        uv_stride: 8,
        y: vec![128; 16 * 16],
        u: vec![128; 8 * 8],
        v: vec![128; 8 * 8],
    };
    assert_eq!(
        encode_dc_predicted_macroblock_key_frame(&source),
        Err(Vp8EncodeError::InvalidDimensions)
    );
}

#[test]
fn dc_predicted_macroblock_key_frame_reconstructs_its_quantized_luma() {
    let source = Vp8SourceYuv {
        width: 16,
        height: 16,
        y_stride: 16,
        uv_stride: 8,
        y: vec![134; 16 * 16],
        u: vec![128; 8 * 8],
        v: vec![128; 8 * 8],
    };
    let encoded = encode_dc_predicted_macroblock_key_frame(&source).unwrap();
    let header = parse_riff_payload(&encoded, None, &DecodeLimits::default()).unwrap();
    let decoded = decode_intra_frame(&encoded, &header, &DecodeLimits::default()).unwrap();
    assert!(decoded.y.iter().all(|&sample| sample == 134));
    assert!(decoded.u.iter().all(|&sample| sample == 128));
    assert!(decoded.v.iter().all(|&sample| sample == 128));
}

#[test]
fn dc_predicted_frame_uses_reconstructed_neighbour_prediction() {
    let source = Vp8SourceYuv {
        width: 32,
        height: 16,
        y_stride: 32,
        uv_stride: 16,
        y: (0..16)
            .flat_map(|_| (0..32).map(|column| if column < 16 { 124 } else { 134 }))
            .collect(),
        u: vec![128; 16 * 8],
        v: vec![128; 16 * 8],
    };
    let encoded = encode_dc_predicted_key_frame_with_quantizer(&source, 0).unwrap();
    let header = parse_riff_payload(&encoded, None, &DecodeLimits::default()).unwrap();
    let decoded = decode_intra_frame(&encoded, &header, &DecodeLimits::default()).unwrap();
    for row in 0..16 {
        assert!(
            decoded.y[row * decoded.y_stride..row * decoded.y_stride + 16]
                .iter()
                .all(|&sample| sample == 124)
        );
        assert!(
            decoded.y[row * decoded.y_stride + 16..row * decoded.y_stride + 32]
                .iter()
                .all(|&sample| sample == 134)
        );
    }
}

#[test]
fn zero_residual_frames_select_macroblock_skip_when_it_is_smaller() {
    let source = Vp8SourceYuv {
        width: 64,
        height: 64,
        y_stride: 64,
        uv_stride: 32,
        y: vec![128; 64 * 64],
        u: vec![128; 32 * 32],
        v: vec![128; 32 * 32],
    };
    let encoded = encode_dc_predicted_key_frame_with_quantizer(&source, 127).unwrap();
    let header = parse_riff_payload(&encoded, None, &DecodeLimits::default()).unwrap();
    let layout =
        crate::vp8::parse_partition_layout(&encoded, &header, &DecodeLimits::default()).unwrap();
    assert!(layout.header.coefficients.use_skip_probability);
    let decoded = decode_intra_frame(&encoded, &header, &DecodeLimits::default()).unwrap();
    assert!(decoded.y.iter().all(|&sample| sample == 128));
    assert!(decoded.u.iter().all(|&sample| sample == 128));
    assert!(decoded.v.iter().all(|&sample| sample == 128));
}

#[test]
fn repeated_coefficient_events_select_frame_probability_updates() {
    let width = 128_usize;
    let height = 128_usize;
    let y = (0..width * height)
        .map(|index| {
            let row = index / width;
            let column = index % width;
            if (row / 4 + column / 4).is_multiple_of(2) {
                48
            } else {
                208
            }
        })
        .collect();
    let source = Vp8SourceYuv {
        width: width as u32,
        height: height as u32,
        y_stride: width,
        uv_stride: width / 2,
        y,
        u: vec![128; width * height / 4],
        v: vec![128; width * height / 4],
    };
    let encoded = encode_dc_predicted_key_frame_with_quantizer(&source, 75).unwrap();
    let header = parse_riff_payload(&encoded, None, &DecodeLimits::default()).unwrap();
    let layout =
        crate::vp8::parse_partition_layout(&encoded, &header, &DecodeLimits::default()).unwrap();
    assert_ne!(
        layout.header.coefficients.values, COEFFICIENT_DEFAULTS,
        "a repeated non-zero token distribution should amortize frame updates"
    );
    let decoded = decode_intra_frame(&encoded, &header, &DecodeLimits::default()).unwrap();
    assert_eq!((decoded.width, decoded.height), (128, 128));
}

#[test]
fn intra16_selector_uses_vertical_prediction_when_top_edge_matches_source() {
    let matrix = crate::vp8::derive_dequantization(
        crate::vp8::QuantizationHeader {
            base_index: 0,
            y1_dc_delta: 0,
            y2_dc_delta: 0,
            y2_ac_delta: 0,
            uv_dc_delta: 0,
            uv_ac_delta: 0,
        },
        &crate::vp8::SegmentHeader {
            enabled: false,
            update_map: false,
            absolute_delta: false,
            quantizer: [0; 4],
            filter_strength: [0; 4],
            probabilities: [255; 3],
        },
    )[0];
    let top_y = std::array::from_fn(|column| 32 + column as u8 * 8);
    let y: [u8; 256] = std::array::from_fn(|index| top_y[index % 16]);
    let (block, coefficients, _) = select_intra16_macroblock(
        &y,
        16,
        &[128; 64],
        &[128; 64],
        8,
        matrix,
        crate::vp8::MacroblockPredictionEdges {
            top_y: Some(top_y),
            top_u: Some([128; 8]),
            top_v: Some([128; 8]),
            ..crate::vp8::MacroblockPredictionEdges::default()
        },
    )
    .unwrap();
    assert_eq!(
        block.luma,
        crate::vp8::LumaMode::Sixteen(crate::vp8::Intra16Mode::Vertical)
    );
    assert!(coefficients.y2.iter().all(|&value| value == 0));
    assert!(coefficients.luma.iter().flatten().all(|&value| value == 0));
}

#[test]
fn factored_intra16_search_matches_exhaustive_mode_pairs() {
    let matrix = crate::vp8::derive_dequantization(
        crate::vp8::QuantizationHeader {
            base_index: 37,
            y1_dc_delta: 0,
            y2_dc_delta: 0,
            y2_ac_delta: 0,
            uv_dc_delta: 0,
            uv_ac_delta: 0,
        },
        &crate::vp8::SegmentHeader {
            enabled: false,
            update_map: false,
            absolute_delta: false,
            quantizer: [0; 4],
            filter_strength: [0; 4],
            probabilities: [255; 3],
        },
    )[0];
    let y: [u8; 256] = std::array::from_fn(|index| {
        let row = index / 16;
        let column = index % 16;
        (row * 11 + column * 7 + 23) as u8
    });
    let u: [u8; 64] = std::array::from_fn(|index| (index * 13 + 41) as u8);
    let v: [u8; 64] = std::array::from_fn(|index| (index * 17 + 19) as u8);
    let edges = crate::vp8::MacroblockPredictionEdges {
        top_y: Some(std::array::from_fn(|index| 31 + index as u8 * 9)),
        left_y: Some(std::array::from_fn(|index| {
            211_u8.wrapping_sub(index as u8 * 5)
        })),
        top_left_y: 97,
        top_u: Some(std::array::from_fn(|index| 61 + index as u8 * 8)),
        left_u: Some(std::array::from_fn(|index| {
            181_u8.wrapping_sub(index as u8 * 7)
        })),
        top_left_u: 101,
        top_v: Some(std::array::from_fn(|index| 37 + index as u8 * 11)),
        left_v: Some(std::array::from_fn(|index| {
            203_u8.wrapping_sub(index as u8 * 9)
        })),
        top_left_v: 89,
        ..crate::vp8::MacroblockPredictionEdges::default()
    };
    let factored = select_intra16_macroblock(&y, 16, &u, &v, 8, matrix, edges).unwrap();
    let exhaustive = exhaustive_intra16_search(&y, &u, &v, matrix, edges);
    assert_eq!(factored, exhaustive);
}

fn exhaustive_intra16_search(
    y: &[u8; 256],
    u: &[u8; 64],
    v: &[u8; 64],
    matrix: crate::vp8::DequantizationMatrix,
    edges: crate::vp8::MacroblockPredictionEdges,
) -> (
    IntraMacroblock,
    Vp8DcMacroblockCoefficients,
    crate::vp8::MacroblockPixels,
) {
    let mut best = None;
    for luma_mode in [
        crate::vp8::Intra16Mode::Dc,
        crate::vp8::Intra16Mode::Vertical,
        crate::vp8::Intra16Mode::Horizontal,
        crate::vp8::Intra16Mode::TrueMotion,
    ] {
        for chroma_mode in [
            crate::vp8::ChromaMode::Dc,
            crate::vp8::ChromaMode::Vertical,
            crate::vp8::ChromaMode::Horizontal,
            crate::vp8::ChromaMode::TrueMotion,
        ] {
            let block = IntraMacroblock {
                segment: 0,
                skip: false,
                luma: crate::vp8::LumaMode::Sixteen(luma_mode),
                chroma: chroma_mode,
            };
            let prediction = predict_intra16_macroblock(luma_mode, chroma_mode, edges);
            let coefficients =
                quantize_intra16_macroblock(y, 16, u, v, 8, prediction, matrix).unwrap();
            let pixels = reconstruct_intra_macroblock(
                block,
                &dc_macroblock_residuals(coefficients),
                matrix,
                edges,
            )
            .unwrap();
            let score = (
                luma_distortion(y, 16, &pixels.y)
                    + chroma_distortion(u, v, 8, &pixels.u, &pixels.v),
                luma_coefficient_cost(coefficients.y2, coefficients.luma)
                    + chroma_coefficient_cost(coefficients.u, coefficients.v),
            );
            if best.is_none_or(|(best_score, _, _, _)| score < best_score) {
                best = Some((score, block, coefficients, pixels));
            }
        }
    }
    best.map(|(_, block, coefficients, pixels)| (block, coefficients, pixels))
        .unwrap()
}
