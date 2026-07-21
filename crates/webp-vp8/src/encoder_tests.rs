use super::*;
use crate::{decode_intra_frame, parse_riff_payload};
use webp_core::DecodeLimits;

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
    let matrix = crate::derive_dequantization(
        crate::QuantizationHeader {
            base_index: 0,
            y1_dc_delta: 0,
            y2_dc_delta: 0,
            y2_ac_delta: 0,
            uv_dc_delta: 0,
            uv_ac_delta: 0,
        },
        &crate::SegmentHeader {
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
    let matrix = crate::derive_dequantization(
        crate::QuantizationHeader {
            base_index: 0,
            y1_dc_delta: 0,
            y2_dc_delta: 0,
            y2_ac_delta: 0,
            uv_dc_delta: 0,
            uv_ac_delta: 0,
        },
        &crate::SegmentHeader {
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
        assert!(decoded.y[row * decoded.y_stride..row * decoded.y_stride + 16]
            .iter()
            .all(|&sample| sample == 124));
        assert!(decoded.y[row * decoded.y_stride + 16..row * decoded.y_stride + 32]
            .iter()
            .all(|&sample| sample == 134));
    }
}

#[test]
fn intra16_selector_uses_vertical_prediction_when_top_edge_matches_source() {
    let matrix = crate::derive_dequantization(
        crate::QuantizationHeader {
            base_index: 0,
            y1_dc_delta: 0,
            y2_dc_delta: 0,
            y2_ac_delta: 0,
            uv_dc_delta: 0,
            uv_ac_delta: 0,
        },
        &crate::SegmentHeader {
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
        crate::MacroblockPredictionEdges {
            top_y: Some(top_y),
            top_u: Some([128; 8]),
            top_v: Some([128; 8]),
            ..crate::MacroblockPredictionEdges::default()
        },
    )
    .unwrap();
    assert_eq!(block.luma, crate::LumaMode::Sixteen(crate::Intra16Mode::Vertical));
    assert!(coefficients.y2.iter().all(|&value| value == 0));
    assert!(coefficients.luma.iter().flatten().all(|&value| value == 0));
}
