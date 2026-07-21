//! Canonical VP8 quantizer lookup tables from RFC 6386 section 14.1.

use crate::partition::SegmentHeader;

pub const DC: [u16; 128] = [
    4, 5, 6, 7, 8, 9, 10, 10, 11, 12, 13, 14, 15, 16, 17, 17, 18, 19, 20, 20, 21, 21, 22, 22, 23,
    23, 24, 25, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 37, 38, 39, 40, 41, 42, 43, 44,
    45, 46, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64, 65, 66, 67,
    68, 69, 70, 71, 72, 73, 74, 75, 76, 76, 77, 78, 79, 80, 81, 82, 83, 84, 85, 86, 87, 88, 89, 91,
    93, 95, 96, 98, 100, 101, 102, 104, 106, 108, 110, 112, 114, 116, 118, 122, 124, 126, 128, 130,
    132, 134, 136, 138, 140, 143, 145, 148, 151, 154, 157,
];
pub const AC: [u16; 128] = [
    4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28,
    29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52,
    53, 54, 55, 56, 57, 58, 60, 62, 64, 66, 68, 70, 72, 74, 76, 78, 80, 82, 84, 86, 88, 90, 92, 94,
    96, 98, 100, 102, 104, 106, 108, 110, 112, 114, 116, 119, 122, 125, 128, 131, 134, 137, 140,
    143, 146, 149, 152, 155, 158, 161, 164, 167, 170, 173, 177, 181, 185, 189, 193, 197, 201, 205,
    209, 213, 217, 221, 225, 229, 234, 239, 245, 249, 254, 259, 264, 269, 274, 279, 284,
];

/// Quantizer controls carried by the first VP8 partition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QuantizationHeader {
    pub base_index: u8,
    pub y1_dc_delta: i32,
    pub y2_dc_delta: i32,
    pub y2_ac_delta: i32,
    pub uv_dc_delta: i32,
    pub uv_ac_delta: i32,
}

/// Dequantization multipliers for one VP8 segment.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DequantizationMatrix {
    pub y1_dc: u16,
    pub y1_ac: u16,
    pub y2_dc: u16,
    pub y2_ac: u16,
    pub uv_dc: u16,
    pub uv_ac: u16,
    /// Unclamped UV AC index, retained for later dithering decisions.
    pub uv_quant: i32,
}

/// Derives the four VP8 scalar dequantization matrices from first-partition
/// quantizer and segmentation controls.
///
/// When segmentation is disabled, all four output entries equal segment zero,
/// exactly matching VP8's state inheritance rule.
#[must_use]
pub fn derive_dequantization(
    quantization: QuantizationHeader,
    segments: &SegmentHeader,
) -> [DequantizationMatrix; 4] {
    let base = i32::from(quantization.base_index);
    let mut matrices = [DequantizationMatrix {
        y1_dc: 0,
        y1_ac: 0,
        y2_dc: 0,
        y2_ac: 0,
        uv_dc: 0,
        uv_ac: 0,
        uv_quant: 0,
    }; 4];
    for (segment, matrix) in matrices.iter_mut().enumerate() {
        let index = if segments.enabled {
            let segment_quantizer = segments.quantizer[segment];
            if segments.absolute_delta {
                segment_quantizer
            } else {
                base + segment_quantizer
            }
        } else {
            base
        };
        *matrix = dequantization_matrix(index, quantization);
    }
    matrices
}

fn dequantization_matrix(index: i32, quantization: QuantizationHeader) -> DequantizationMatrix {
    let y1_dc = DC[clamp_quantizer(index + quantization.y1_dc_delta, 127)];
    let y1_ac = AC[clamp_quantizer(index, 127)];
    let y2_dc = DC[clamp_quantizer(index + quantization.y2_dc_delta, 127)] * 2;
    let y2_ac = ((u32::from(AC[clamp_quantizer(index + quantization.y2_ac_delta, 127)]) * 101_581)
        >> 16)
        .max(8) as u16;
    let uv_dc = DC[clamp_quantizer(index + quantization.uv_dc_delta, 117)];
    let uv_quant = index + quantization.uv_ac_delta;
    let uv_ac = AC[clamp_quantizer(uv_quant, 127)];
    DequantizationMatrix {
        y1_dc,
        y1_ac,
        y2_dc,
        y2_ac,
        uv_dc,
        uv_ac,
        uv_quant,
    }
}

fn clamp_quantizer(index: i32, maximum: usize) -> usize {
    index.clamp(0, maximum as i32) as usize
}

#[cfg(test)]
#[path = "quantization_tests.rs"]
mod tests;
