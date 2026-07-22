use super::*;

#[test]
fn quantizes_dc_and_ac_with_symmetric_rounding_and_vp8_bounds() {
    let coefficients = [-9, 7, -1, 20_000, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    let quantized = quantize_block(coefficients, 4, 3);
    assert_eq!(quantized[0], -2);
    assert_eq!(quantized[1], 2);
    assert_eq!(quantized[2], 0);
    assert_eq!(quantized[3], 2_047);
}
use crate::vp8::SegmentHeader;

#[test]
fn derives_default_dequantization_for_each_disabled_segment() {
    let matrices = derive_dequantization(
        QuantizationHeader {
            base_index: 0,
            y1_dc_delta: 0,
            y2_dc_delta: 0,
            y2_ac_delta: 0,
            uv_dc_delta: 0,
            uv_ac_delta: 0,
        },
        &SegmentHeader {
            enabled: false,
            update_map: false,
            absolute_delta: true,
            quantizer: [0; 4],
            filter_strength: [0; 4],
            probabilities: [255; 3],
        },
    );
    let expected = DequantizationMatrix {
        y1_dc: 4,
        y1_ac: 4,
        y2_dc: 8,
        y2_ac: 8,
        uv_dc: 4,
        uv_ac: 4,
        uv_quant: 0,
    };
    assert_eq!(matrices, [expected; 4]);
}

#[test]
fn derives_segment_delta_and_absolute_dequantization_with_clamps() {
    let quantization = QuantizationHeader {
        base_index: 126,
        y1_dc_delta: 7,
        y2_dc_delta: -7,
        y2_ac_delta: 7,
        uv_dc_delta: 7,
        uv_ac_delta: -7,
    };
    let delta = derive_dequantization(
        quantization,
        &SegmentHeader {
            enabled: true,
            update_map: false,
            absolute_delta: false,
            quantizer: [2, -127, 0, 1],
            filter_strength: [0; 4],
            probabilities: [255; 3],
        },
    );
    assert_eq!(delta[0].y1_dc, 157);
    assert_eq!(delta[0].y1_ac, 284);
    assert_eq!(delta[0].uv_dc, 132);
    assert_eq!(delta[0].uv_ac, 254);
    assert_eq!(delta[0].uv_quant, 121);
    assert_eq!(
        delta[1],
        DequantizationMatrix {
            y1_dc: 10,
            y1_ac: 4,
            y2_dc: 8,
            y2_ac: 15,
            uv_dc: 10,
            uv_ac: 4,
            uv_quant: -8,
        }
    );

    let absolute = derive_dequantization(
        quantization,
        &SegmentHeader {
            enabled: true,
            update_map: false,
            absolute_delta: true,
            quantizer: [-5, 5, 127, 0],
            filter_strength: [0; 4],
            probabilities: [255; 3],
        },
    );
    assert_eq!(absolute[0].uv_quant, -12);
    assert_eq!(absolute[0].y1_ac, 4);
    assert_eq!(absolute[2].y1_ac, 284);
    assert_eq!(absolute[2].uv_dc, 132);
}

#[test]
fn quantizer_clamp_respects_codec_specific_upper_bounds() {
    assert_eq!(clamp_quantizer(-1, 127), 0);
    assert_eq!(clamp_quantizer(128, 127), 127);
    assert_eq!(clamp_quantizer(118, 117), 117);
}
