//! VP8 intra prediction modes, row parsing, and fixed probabilities.
//!
//! Values are mechanically transcribed from the pinned libwebp reference
//! implementation and shape-checked during this import.

use crate::DecodeError;
use crate::DecodeErrorKind;

use crate::vp8::BoolDecoder;
use crate::vp8::FirstPartitionHeader;
pub use webp_dsp::ChromaMode;
pub use webp_dsp::Intra4Mode;
pub use webp_dsp::Intra16Mode;

/// The luma prediction choice for one VP8 intra macroblock.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LumaMode {
    /// One prediction mode covers the full 16×16 luma macroblock.
    Sixteen(Intra16Mode),
    /// Each luma 4×4 block supplies its own prediction mode in raster order.
    FourByFour([Intra4Mode; 16]),
}

/// Intra controls parsed for one VP8 macroblock.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IntraMacroblock {
    /// Segment selected by the first-partition segment map.
    pub segment: u8,
    /// `true` means this macroblock carries no residual coefficients.
    pub skip: bool,
    pub luma: LumaMode,
    pub chroma: ChromaMode,
}

/// Parses a VP8 intra-mode row without allocating decoder-owned macroblock state.
///
/// `top_modes` stores four luma 4×4 contexts per macroblock from the preceding
/// row; it is updated in place for the row just parsed. For the first row,
/// initialise it to [`Intra4Mode::Dc`]. `blocks` receives one result per
/// macroblock. Both slices must describe the same width (`top_modes.len() ==
/// blocks.len() * 4`). The caller resets no left contexts: VP8 specifies DC
/// contexts at the start of every macroblock row.
pub fn parse_intra_mode_row(
    bits: &mut BoolDecoder<'_>,
    header: &FirstPartitionHeader,
    top_modes: &mut [Intra4Mode],
    blocks: &mut [IntraMacroblock],
) -> Result<(), DecodeError> {
    if top_modes.len() != blocks.len().saturating_mul(4) {
        return Err(DecodeError::at(
            DecodeErrorKind::InvalidParameter,
            bits.bytes_consumed(),
            "VP8 intra-mode top context length must equal four modes per macroblock",
        ));
    }

    let mut left = [Intra4Mode::Dc; 4];
    for (macroblock_index, block) in blocks.iter_mut().enumerate() {
        let top = &mut top_modes[macroblock_index * 4..macroblock_index * 4 + 4];
        let segment = if header.segments.update_map {
            if !bits.read_bool(header.segments.probabilities[0])? {
                u8::from(bits.read_bool(header.segments.probabilities[1])?)
            } else {
                2 + u8::from(bits.read_bool(header.segments.probabilities[2])?)
            }
        } else {
            0
        };
        let skip = header.coefficients.use_skip_probability
            && bits.read_bool(header.coefficients.skip_probability)?;
        let luma = if bits.read_bool(145)? {
            let mode = decode_luma16_mode(bits)?;
            top.fill(intra16_context(mode));
            left.fill(intra16_context(mode));
            LumaMode::Sixteen(mode)
        } else {
            let mut modes = [Intra4Mode::Dc; 16];
            for row in 0..4 {
                let mut mode = left[row];
                for column in 0..4 {
                    mode = decode_intra4_mode(
                        bits,
                        B_MODE_PROBABILITIES[top[column] as usize][mode as usize],
                    )?;
                    top[column] = mode;
                    modes[row * 4 + column] = mode;
                }
                left[row] = mode;
            }
            LumaMode::FourByFour(modes)
        };
        *block = IntraMacroblock {
            segment,
            skip,
            luma,
            chroma: decode_chroma_mode(bits)?,
        };
    }
    Ok(())
}

const fn intra16_context(mode: Intra16Mode) -> Intra4Mode {
    match mode {
        Intra16Mode::Dc => Intra4Mode::Dc,
        Intra16Mode::Vertical => Intra4Mode::Vertical,
        Intra16Mode::Horizontal => Intra4Mode::Horizontal,
        Intra16Mode::TrueMotion => Intra4Mode::TrueMotion,
    }
}

fn decode_luma16_mode(bits: &mut BoolDecoder<'_>) -> Result<Intra16Mode, DecodeError> {
    if bits.read_bool(156)? {
        if bits.read_bool(128)? {
            Ok(Intra16Mode::TrueMotion)
        } else {
            Ok(Intra16Mode::Horizontal)
        }
    } else if bits.read_bool(163)? {
        Ok(Intra16Mode::Vertical)
    } else {
        Ok(Intra16Mode::Dc)
    }
}

fn decode_intra4_mode(
    bits: &mut BoolDecoder<'_>,
    probabilities: [u8; 9],
) -> Result<Intra4Mode, DecodeError> {
    if !bits.read_bool(probabilities[0])? {
        return Ok(Intra4Mode::Dc);
    }
    if !bits.read_bool(probabilities[1])? {
        return Ok(Intra4Mode::TrueMotion);
    }
    if !bits.read_bool(probabilities[2])? {
        return Ok(Intra4Mode::Vertical);
    }
    if !bits.read_bool(probabilities[3])? {
        return if !bits.read_bool(probabilities[4])? {
            Ok(Intra4Mode::Horizontal)
        } else if !bits.read_bool(probabilities[5])? {
            Ok(Intra4Mode::DiagonalDownRight)
        } else {
            Ok(Intra4Mode::VerticalRight)
        };
    }
    if !bits.read_bool(probabilities[6])? {
        return Ok(Intra4Mode::DiagonalDownLeft);
    }
    if !bits.read_bool(probabilities[7])? {
        return Ok(Intra4Mode::VerticalLeft);
    }
    if !bits.read_bool(probabilities[8])? {
        Ok(Intra4Mode::HorizontalDown)
    } else {
        Ok(Intra4Mode::HorizontalUp)
    }
}

fn decode_chroma_mode(bits: &mut BoolDecoder<'_>) -> Result<ChromaMode, DecodeError> {
    if !bits.read_bool(142)? {
        Ok(ChromaMode::Dc)
    } else if !bits.read_bool(114)? {
        Ok(ChromaMode::Vertical)
    } else if bits.read_bool(183)? {
        Ok(ChromaMode::TrueMotion)
    } else {
        Ok(ChromaMode::Horizontal)
    }
}

pub const B_MODE_PROBABILITIES: [[[u8; 9]; 10]; 10] = [
    [
        [231, 120, 48, 89, 115, 113, 120, 152, 112],
        [152, 179, 64, 126, 170, 118, 46, 70, 95],
        [175, 69, 143, 80, 85, 82, 72, 155, 103],
        [56, 58, 10, 171, 218, 189, 17, 13, 152],
        [114, 26, 17, 163, 44, 195, 21, 10, 173],
        [121, 24, 80, 195, 26, 62, 44, 64, 85],
        [144, 71, 10, 38, 171, 213, 144, 34, 26],
        [170, 46, 55, 19, 136, 160, 33, 206, 71],
        [63, 20, 8, 114, 114, 208, 12, 9, 226],
        [81, 40, 11, 96, 182, 84, 29, 16, 36],
    ],
    [
        [134, 183, 89, 137, 98, 101, 106, 165, 148],
        [72, 187, 100, 130, 157, 111, 32, 75, 80],
        [66, 102, 167, 99, 74, 62, 40, 234, 128],
        [41, 53, 9, 178, 241, 141, 26, 8, 107],
        [74, 43, 26, 146, 73, 166, 49, 23, 157],
        [65, 38, 105, 160, 51, 52, 31, 115, 128],
        [104, 79, 12, 27, 217, 255, 87, 17, 7],
        [87, 68, 71, 44, 114, 51, 15, 186, 23],
        [47, 41, 14, 110, 182, 183, 21, 17, 194],
        [66, 45, 25, 102, 197, 189, 23, 18, 22],
    ],
    [
        [88, 88, 147, 150, 42, 46, 45, 196, 205],
        [43, 97, 183, 117, 85, 38, 35, 179, 61],
        [39, 53, 200, 87, 26, 21, 43, 232, 171],
        [56, 34, 51, 104, 114, 102, 29, 93, 77],
        [39, 28, 85, 171, 58, 165, 90, 98, 64],
        [34, 22, 116, 206, 23, 34, 43, 166, 73],
        [107, 54, 32, 26, 51, 1, 81, 43, 31],
        [68, 25, 106, 22, 64, 171, 36, 225, 114],
        [34, 19, 21, 102, 132, 188, 16, 76, 124],
        [62, 18, 78, 95, 85, 57, 50, 48, 51],
    ],
    [
        [193, 101, 35, 159, 215, 111, 89, 46, 111],
        [60, 148, 31, 172, 219, 228, 21, 18, 111],
        [112, 113, 77, 85, 179, 255, 38, 120, 114],
        [40, 42, 1, 196, 245, 209, 10, 25, 109],
        [88, 43, 29, 140, 166, 213, 37, 43, 154],
        [61, 63, 30, 155, 67, 45, 68, 1, 209],
        [100, 80, 8, 43, 154, 1, 51, 26, 71],
        [142, 78, 78, 16, 255, 128, 34, 197, 171],
        [41, 40, 5, 102, 211, 183, 4, 1, 221],
        [51, 50, 17, 168, 209, 192, 23, 25, 82],
    ],
    [
        [138, 31, 36, 171, 27, 166, 38, 44, 229],
        [67, 87, 58, 169, 82, 115, 26, 59, 179],
        [63, 59, 90, 180, 59, 166, 93, 73, 154],
        [40, 40, 21, 116, 143, 209, 34, 39, 175],
        [47, 15, 16, 183, 34, 223, 49, 45, 183],
        [46, 17, 33, 183, 6, 98, 15, 32, 183],
        [57, 46, 22, 24, 128, 1, 54, 17, 37],
        [65, 32, 73, 115, 28, 128, 23, 128, 205],
        [40, 3, 9, 115, 51, 192, 18, 6, 223],
        [87, 37, 9, 115, 59, 77, 64, 21, 47],
    ],
    [
        [104, 55, 44, 218, 9, 54, 53, 130, 226],
        [64, 90, 70, 205, 40, 41, 23, 26, 57],
        [54, 57, 112, 184, 5, 41, 38, 166, 213],
        [30, 34, 26, 133, 152, 116, 10, 32, 134],
        [39, 19, 53, 221, 26, 114, 32, 73, 255],
        [31, 9, 65, 234, 2, 15, 1, 118, 73],
        [75, 32, 12, 51, 192, 255, 160, 43, 51],
        [88, 31, 35, 67, 102, 85, 55, 186, 85],
        [56, 21, 23, 111, 59, 205, 45, 37, 192],
        [55, 38, 70, 124, 73, 102, 1, 34, 98],
    ],
    [
        [125, 98, 42, 88, 104, 85, 117, 175, 82],
        [95, 84, 53, 89, 128, 100, 113, 101, 45],
        [75, 79, 123, 47, 51, 128, 81, 171, 1],
        [57, 17, 5, 71, 102, 57, 53, 41, 49],
        [38, 33, 13, 121, 57, 73, 26, 1, 85],
        [41, 10, 67, 138, 77, 110, 90, 47, 114],
        [115, 21, 2, 10, 102, 255, 166, 23, 6],
        [101, 29, 16, 10, 85, 128, 101, 196, 26],
        [57, 18, 10, 102, 102, 213, 34, 20, 43],
        [117, 20, 15, 36, 163, 128, 68, 1, 26],
    ],
    [
        [102, 61, 71, 37, 34, 53, 31, 243, 192],
        [69, 60, 71, 38, 73, 119, 28, 222, 37],
        [68, 45, 128, 34, 1, 47, 11, 245, 171],
        [62, 17, 19, 70, 146, 85, 55, 62, 70],
        [37, 43, 37, 154, 100, 163, 85, 160, 1],
        [63, 9, 92, 136, 28, 64, 32, 201, 85],
        [75, 15, 9, 9, 64, 255, 184, 119, 16],
        [86, 6, 28, 5, 64, 255, 25, 248, 1],
        [56, 8, 17, 132, 137, 255, 55, 116, 128],
        [58, 15, 20, 82, 135, 57, 26, 121, 40],
    ],
    [
        [164, 50, 31, 137, 154, 133, 25, 35, 218],
        [51, 103, 44, 131, 131, 123, 31, 6, 158],
        [86, 40, 64, 135, 148, 224, 45, 183, 128],
        [22, 26, 17, 131, 240, 154, 14, 1, 209],
        [45, 16, 21, 91, 64, 222, 7, 1, 197],
        [56, 21, 39, 155, 60, 138, 23, 102, 213],
        [83, 12, 13, 54, 192, 255, 68, 47, 28],
        [85, 26, 85, 85, 128, 128, 32, 146, 171],
        [18, 11, 7, 63, 144, 171, 4, 4, 246],
        [35, 27, 10, 146, 174, 171, 12, 26, 128],
    ],
    [
        [190, 80, 35, 99, 180, 80, 126, 54, 45],
        [85, 126, 47, 87, 176, 51, 41, 20, 32],
        [101, 75, 128, 139, 118, 146, 116, 128, 85],
        [56, 41, 15, 176, 236, 85, 37, 9, 62],
        [71, 30, 17, 119, 118, 255, 17, 18, 138],
        [101, 38, 60, 138, 55, 70, 43, 26, 142],
        [146, 36, 19, 30, 171, 255, 97, 27, 20],
        [138, 45, 61, 62, 219, 1, 81, 188, 64],
        [32, 41, 20, 117, 151, 142, 20, 21, 163],
        [112, 19, 12, 61, 195, 128, 48, 4, 24],
    ],
];

#[cfg(test)]
#[path = "intra_prediction_tests.rs"]
mod tests;
