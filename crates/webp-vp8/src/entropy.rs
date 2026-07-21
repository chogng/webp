//! VP8 coefficient-token entropy decoding and residual context.

use webp_core::{DecodeError, DecodeErrorKind};

use crate::BoolDecoder;
use crate::coefficients::{
    CATEGORY_PROBABILITIES, COEFFICIENT_BANDS, COEFFICIENT_DEFAULTS, COEFFICIENT_ZIGZAG,
};

/// Canonical VP8 coefficient probabilities after first-partition updates.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoefficientProbabilities {
    pub(crate) values: [[[[u8; 11]; 3]; 8]; 4],
    pub use_skip_probability: bool,
    pub skip_probability: u8,
}

impl Default for CoefficientProbabilities {
    fn default() -> Self {
        Self {
            values: COEFFICIENT_DEFAULTS,
            use_skip_probability: false,
            skip_probability: 0,
        }
    }
}

impl CoefficientProbabilities {
    /// Reads one canonical probability by coefficient type, band, context, and
    /// tree node. All indices are specification-bounded: `4 × 8 × 3 × 11`.
    #[must_use]
    pub fn get(&self, coefficient_type: usize, band: usize, context: usize, node: usize) -> u8 {
        self.values[coefficient_type][band][context][node]
    }

    pub(crate) fn nodes(
        &self,
        coefficient_type: CoefficientBlockType,
        position: usize,
        context: usize,
    ) -> &[u8; 11] {
        &self.values[coefficient_type as usize][COEFFICIENT_BANDS[position]][context]
    }
}

/// One VP8 coefficient probability family.
///
/// The variants follow the bitstream's `NUM_TYPES` ordering, rather than the
/// order in which macroblock reconstruction consumes them.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum CoefficientBlockType {
    /// Luma AC for a 16×16-predicted macroblock.
    Luma16Ac = 0,
    /// The macroblock's Y2 DC transform block.
    LumaDc = 1,
    /// Chroma AC block.
    ChromaAc = 2,
    /// Luma AC for one 4×4-predicted block.
    Luma4Ac = 3,
}

/// Quantized values decoded from one VP8 4×4 coefficient token stream.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DecodedCoefficients {
    /// Quantized signed levels, stored in raster order and not dequantized.
    pub values: [i16; 16],
    /// Number of entropy positions consumed before EOB or the block boundary.
    pub end: u8,
    /// Number of non-zero coefficients in [`Self::values`].
    pub non_zero: u8,
}

/// Decodes a VP8 coefficient-token stream for one 4×4 block.
///
/// `context` is the preceding-neighbour non-zero context (`0..=2`), and
/// `start` selects the first entropy position (`0` normally, or `1` for the
/// AC-only portion of a luma 16×16 block). Returned values are deliberately
/// left quantized: a later macroblock stage owns dequantization and can apply
/// the corresponding Y1/Y2/UV matrix without losing overflow information.
pub fn decode_coefficients(
    bits: &mut BoolDecoder<'_>,
    probabilities: &CoefficientProbabilities,
    coefficient_type: CoefficientBlockType,
    context: u8,
    start: u8,
) -> Result<DecodedCoefficients, DecodeError> {
    if context > 2 {
        return Err(DecodeError::at(
            DecodeErrorKind::InvalidParameter,
            bits.bytes_consumed(),
            "VP8 coefficient context must be in 0..=2",
        ));
    }
    let mut position = usize::from(start);
    if position >= 16 {
        return Err(DecodeError::at(
            DecodeErrorKind::InvalidParameter,
            bits.bytes_consumed(),
            "VP8 coefficient start must be in 0..=15",
        ));
    }

    let mut values = [0_i16; 16];
    let mut non_zero = 0_u8;
    let mut nodes = probabilities.nodes(coefficient_type, position, usize::from(context));
    while position < 16 {
        if !bits.read_bool(nodes[0])? {
            break;
        }
        while !bits.read_bool(nodes[1])? {
            position += 1;
            if position == 16 {
                return Ok(DecodedCoefficients {
                    values,
                    end: 16,
                    non_zero,
                });
            }
            nodes = probabilities.nodes(coefficient_type, position, 0);
        }

        let magnitude = if !bits.read_bool(nodes[2])? {
            1
        } else {
            decode_large_coefficient(bits, nodes)?
        };
        let next_context = if magnitude == 1 { 1 } else { 2 };
        let negative = bits.read_bool(128)?;
        let signed = if negative { -magnitude } else { magnitude };
        values[COEFFICIENT_ZIGZAG[position]] = signed;
        non_zero += 1;
        position += 1;
        if position < 16 {
            nodes = probabilities.nodes(coefficient_type, position, next_context);
        }
    }

    Ok(DecodedCoefficients {
        values,
        end: position as u8,
        non_zero,
    })
}

fn decode_large_coefficient(
    bits: &mut BoolDecoder<'_>,
    nodes: &[u8; 11],
) -> Result<i16, DecodeError> {
    let value = if !bits.read_bool(nodes[3])? {
        if !bits.read_bool(nodes[4])? {
            2
        } else {
            3 + i16::from(bits.read_bool(nodes[5])?)
        }
    } else if !bits.read_bool(nodes[6])? {
        if !bits.read_bool(nodes[7])? {
            5 + i16::from(bits.read_bool(159)?)
        } else {
            7 + 2 * i16::from(bits.read_bool(165)?) + i16::from(bits.read_bool(145)?)
        }
    } else {
        let high = usize::from(bits.read_bool(nodes[8])?);
        let low = usize::from(bits.read_bool(nodes[9 + high])?);
        let category = high * 2 + low;
        let mut suffix = 0_i16;
        for &probability in CATEGORY_PROBABILITIES[category] {
            suffix = (suffix << 1) | i16::from(bits.read_bool(probability)?);
        }
        suffix + 3 + (8_i16 << category)
    };
    Ok(value)
}

/// Non-zero-token context retained between neighbouring VP8 macroblocks.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ResidualContext {
    /// Four luma and four chroma edge contexts packed in VP8 scan order.
    pub non_zero: u8,
    /// The Y2 DC-transform context for 16×16-predicted macroblocks.
    pub non_zero_dc: bool,
}

/// Quantized residual token data for one VP8 macroblock.
///
/// The coefficients remain quantized. Reconstruction owns dequantization and
/// transform selection so it can use the macroblock's chosen segment matrix.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MacroblockResiduals {
    /// Present only for a 16×16 luma prediction; it carries the Y2 DC block.
    pub y2: Option<DecodedCoefficients>,
    /// Sixteen luma 4×4 blocks in raster order.
    pub luma: [DecodedCoefficients; 16],
    /// Four U 4×4 blocks in raster order.
    pub u: [DecodedCoefficients; 4],
    /// Four V 4×4 blocks in raster order.
    pub v: [DecodedCoefficients; 4],
    /// VP8's compact two-bit luma transform-selection flags.
    pub non_zero_y: u32,
    /// VP8's compact two-bit chroma transform-selection flags.
    pub non_zero_uv: u32,
}

/// Decodes one intra macroblock's coefficient-token residuals.
///
/// `top` and `left` retain the non-zero contexts from already-decoded
/// neighbours and are updated in place. `is_i4x4` must match the corresponding
/// [`IntraMacroblock`]'s luma mode. This function consumes only the selected
/// token-partition boolean decoder and does not allocate.
pub fn decode_intra_residuals(
    bits: &mut BoolDecoder<'_>,
    probabilities: &CoefficientProbabilities,
    is_i4x4: bool,
    top: &mut ResidualContext,
    left: &mut ResidualContext,
) -> Result<MacroblockResiduals, DecodeError> {
    let empty = DecodedCoefficients {
        values: [0; 16],
        end: 0,
        non_zero: 0,
    };
    let mut residuals = MacroblockResiduals {
        y2: None,
        luma: [empty; 16],
        u: [empty; 4],
        v: [empty; 4],
        non_zero_y: 0,
        non_zero_uv: 0,
    };

    let (first, luma_type) = if is_i4x4 {
        (0, CoefficientBlockType::Luma4Ac)
    } else {
        let y2_context = u8::from(top.non_zero_dc) + u8::from(left.non_zero_dc);
        let y2 = decode_coefficients(
            bits,
            probabilities,
            CoefficientBlockType::LumaDc,
            y2_context,
            0,
        )?;
        let present = y2.end > 0;
        top.non_zero_dc = present;
        left.non_zero_dc = present;
        residuals.y2 = Some(y2);
        (1, CoefficientBlockType::Luma16Ac)
    };

    let mut top_non_zero = top.non_zero & 0x0f;
    let mut left_non_zero = left.non_zero & 0x0f;
    for row in 0..4 {
        let mut left_block_non_zero = left_non_zero & 1;
        let mut row_flags = 0_u32;
        for column in 0..4 {
            let context = left_block_non_zero + (top_non_zero & 1);
            let coefficients = decode_coefficients(bits, probabilities, luma_type, context, first)?;
            left_block_non_zero = u8::from(coefficients.end > first);
            top_non_zero = (top_non_zero >> 1) | (left_block_non_zero << 7);
            row_flags = non_zero_code(row_flags, coefficients);
            residuals.luma[row * 4 + column] = coefficients;
        }
        top_non_zero >>= 4;
        left_non_zero = (left_non_zero >> 1) | (left_block_non_zero << 7);
        residuals.non_zero_y = (residuals.non_zero_y << 8) | row_flags;
    }
    let mut output_top = top_non_zero;
    let mut output_left = left_non_zero >> 4;

    for chroma_plane in 0..2 {
        let shift = 4 + chroma_plane * 2;
        top_non_zero = top.non_zero >> shift;
        left_non_zero = left.non_zero >> shift;
        let destination = if chroma_plane == 0 {
            &mut residuals.u
        } else {
            &mut residuals.v
        };
        let mut plane_flags = 0_u32;
        for row in 0..2 {
            let mut left_block_non_zero = left_non_zero & 1;
            for column in 0..2 {
                let context = left_block_non_zero + (top_non_zero & 1);
                let coefficients = decode_coefficients(
                    bits,
                    probabilities,
                    CoefficientBlockType::ChromaAc,
                    context,
                    0,
                )?;
                left_block_non_zero = u8::from(coefficients.end > 0);
                top_non_zero = (top_non_zero >> 1) | (left_block_non_zero << 3);
                plane_flags = non_zero_code(plane_flags, coefficients);
                destination[row * 2 + column] = coefficients;
            }
            top_non_zero >>= 2;
            left_non_zero = (left_non_zero >> 1) | (left_block_non_zero << 5);
        }
        residuals.non_zero_uv |= plane_flags << (4 * chroma_plane);
        output_top |= (top_non_zero << 4) << (chroma_plane * 2);
        output_left |= (left_non_zero & 0xf0) << (chroma_plane * 2);
    }
    top.non_zero = output_top;
    left.non_zero = output_left;
    Ok(residuals)
}

fn non_zero_code(existing: u32, coefficients: DecodedCoefficients) -> u32 {
    let code = if coefficients.end > 3 {
        3
    } else if coefficients.end > 1 {
        2
    } else if coefficients.values[0] != 0 {
        1
    } else {
        0
    };
    (existing << 2) | code
}
