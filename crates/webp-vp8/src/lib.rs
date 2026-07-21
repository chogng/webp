#![forbid(unsafe_code)]
//! VP8 lossy WebP decoder primitives.
//!
//! This M2 foundation parses the complete uncompressed VP8 key-frame header
//! and validates the first-partition boundary before any entropy state or
//! macroblock storage is allocated.  Pixel decoding is intentionally kept out
//! of this crate's first slice.

use webp_core::{DecodeError, DecodeErrorKind, DecodeLimits, WorkBudget};

mod coefficients;
mod frame;
mod intra;
mod loop_filter;
mod quantization;

use coefficients::{COEFFICIENT_DEFAULTS, COEFFICIENT_UPDATE_PROBABILITIES};
use intra::B_MODE_PROBABILITIES;
use quantization::{AC as DEQUANT_AC, DC as DEQUANT_DC};

pub use frame::{Vp8YuvImage, decode_intra_frame};
pub use loop_filter::{
    LoopFilterStrength, derive_loop_filter_strengths, filter_normal_edge, filter_simple_edge,
};

const FRAME_TAG_LEN: usize = 3;
const KEY_FRAME_HEADER_LEN: usize = 10;
const KEY_FRAME_START_CODE: [u8; 3] = [0x9d, 0x01, 0x2a];

/// VP8's coefficient scan order, mapping entropy positions to raster indexes.
pub const COEFFICIENT_ZIGZAG: [usize; 16] = [0, 1, 4, 8, 5, 2, 3, 6, 9, 12, 13, 10, 7, 11, 14, 15];

// The final sentinel is used while selecting the probability context after
// coefficient 15. It is intentionally zero because that context cannot be
// consumed by a legal 4x4 block, but keeping it mirrors VP8's 17-entry table.
const COEFFICIENT_BANDS: [usize; 17] = [0, 1, 2, 3, 6, 4, 5, 6, 6, 6, 6, 6, 6, 6, 6, 7, 0];

const CATEGORY_PROBABILITIES: [&[u8]; 4] = [
    &[173, 148, 140],
    &[176, 155, 140, 135],
    &[180, 157, 141, 134, 130],
    &[254, 254, 243, 230, 196, 177, 153, 140, 133, 130, 129],
];

/// VP8's most-significant-bit-first arithmetic boolean decoder.
///
/// The decoder owns a deterministic work budget: every decoded boolean value
/// consumes one unit. It never fabricates zero-padding beyond the supplied
/// partition; callers receive [`DecodeErrorKind::UnexpectedEof`] instead.
#[derive(Clone, Debug)]
pub struct BoolDecoder<'a> {
    data: &'a [u8],
    byte_position: usize,
    value: u64,
    /// VP8 stores the active interval as `range - 1`.
    range: u32,
    /// Number of cached low bits usable as the comparison position.
    bits: i32,
    work: WorkBudget,
}

impl<'a> BoolDecoder<'a> {
    /// Creates a decoder over one already-bounded VP8 partition.
    pub fn new(data: &'a [u8], limits: &DecodeLimits) -> Result<Self, DecodeError> {
        limits.check_input_len(data.len())?;
        Ok(Self {
            data,
            byte_position: 0,
            value: 0,
            range: 254,
            bits: -8,
            work: limits.work_budget(),
        })
    }

    /// Decodes one boolean value with the supplied VP8 probability.
    pub fn read_bool(&mut self, probability: u8) -> Result<bool, DecodeError> {
        self.work.consume(1)?;
        if self.bits < 0 {
            self.load_byte()?;
        }

        let split = (self.range * u32::from(probability)) >> 8;
        let value = (self.value >> self.bits) as u32;
        let bit = value > split;
        if bit {
            self.range -= split;
            self.value -= u64::from(split + 1) << self.bits;
        } else {
            self.range = split + 1;
        }

        let shift = 7 - self.range.ilog2() as i32;
        self.range <<= shift;
        self.bits -= shift;
        self.range -= 1;
        Ok(bit)
    }

    /// Reads a fixed-width, most-significant-bit-first VP8 literal.
    pub fn read_literal(&mut self, count: u8) -> Result<u32, DecodeError> {
        if count > 32 {
            return Err(DecodeError::at(
                DecodeErrorKind::InvalidParameter,
                self.byte_position,
                "VP8 literal width exceeds 32 bits",
            ));
        }
        let mut value = 0_u32;
        for _ in 0..count {
            value = (value << 1) | u32::from(self.read_bool(128)?);
        }
        Ok(value)
    }

    /// Reads a VP8 sign-magnitude value: magnitude first, then its sign bit.
    pub fn read_signed_literal(&mut self, count: u8) -> Result<i32, DecodeError> {
        let raw_magnitude = self.read_literal(count)?;
        let magnitude = i32::try_from(raw_magnitude).map_err(|_| {
            DecodeError::at(
                DecodeErrorKind::InvalidBitstream,
                self.byte_position,
                "VP8 signed literal does not fit i32",
            )
        })?;
        if self.read_bool(128)? {
            Ok(-magnitude)
        } else {
            Ok(magnitude)
        }
    }

    /// Number of input bytes consumed from this partition.
    #[must_use]
    pub const fn bytes_consumed(&self) -> usize {
        self.byte_position
    }

    /// Remaining deterministic decoder work units.
    #[must_use]
    pub const fn remaining_work(&self) -> u64 {
        self.work.remaining()
    }

    fn load_byte(&mut self) -> Result<(), DecodeError> {
        let byte = *self.data.get(self.byte_position).ok_or_else(|| {
            DecodeError::at(
                DecodeErrorKind::UnexpectedEof,
                self.byte_position,
                "truncated VP8 boolean-coded partition",
            )
        })?;
        self.byte_position += 1;
        self.value = u64::from(byte) | (self.value << 8);
        self.bits += 8;
        Ok(())
    }
}

/// Segmentation data carried by the first VP8 partition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SegmentHeader {
    pub enabled: bool,
    pub update_map: bool,
    pub absolute_delta: bool,
    pub quantizer: [i32; 4],
    pub filter_strength: [i32; 4],
    pub probabilities: [u8; 3],
}

/// Loop-filter controls carried by the first VP8 partition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FilterHeader {
    pub simple: bool,
    pub level: u8,
    pub sharpness: u8,
    pub use_deltas: bool,
    pub ref_deltas: [i32; 4],
    pub mode_deltas: [i32; 4],
}

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
    let y1_dc = DEQUANT_DC[clamp_quantizer(index + quantization.y1_dc_delta, 127)];
    let y1_ac = DEQUANT_AC[clamp_quantizer(index, 127)];
    let y2_dc = DEQUANT_DC[clamp_quantizer(index + quantization.y2_dc_delta, 127)] * 2;
    let y2_ac = ((u32::from(DEQUANT_AC[clamp_quantizer(index + quantization.y2_ac_delta, 127)])
        * 101_581)
        >> 16)
        .max(8) as u16;
    let uv_dc = DEQUANT_DC[clamp_quantizer(index + quantization.uv_dc_delta, 117)];
    let uv_quant = index + quantization.uv_ac_delta;
    let uv_ac = DEQUANT_AC[clamp_quantizer(uv_quant, 127)];
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

/// Performs VP8's integer inverse 4×4 DCT and returns pixel-domain residues.
///
/// Coefficients are in raster order after dequantization. All intermediates
/// use `i32`, preserving the specification's fixed-point rounding before the
/// final divide by eight.
#[must_use]
pub fn inverse_dct_4x4(coefficients: [i16; 16]) -> [i32; 16] {
    inverse_dct_4x4_i32(coefficients.map(i32::from))
}

/// Performs VP8's integer inverse 4×4 DCT on widened coefficients.
///
/// This is the reconstruction-facing form of [`inverse_dct_4x4`]. It keeps
/// dequantized coefficients in `i32`, so a malformed stream cannot force an
/// intermediate narrowing conversion before prediction and sample clipping.
#[must_use]
pub fn inverse_dct_4x4_i32(coefficients: [i32; 16]) -> [i32; 16] {
    let mut temporary = [0_i32; 16];
    for column in 0..4 {
        let a = coefficients[column] + coefficients[8 + column];
        let b = coefficients[column] - coefficients[8 + column];
        let c = transform_mul2_i32(coefficients[4 + column])
            - transform_mul1_i32(coefficients[12 + column]);
        let d = transform_mul1_i32(coefficients[4 + column])
            + transform_mul2_i32(coefficients[12 + column]);
        temporary[column * 4] = a + d;
        temporary[column * 4 + 1] = b + c;
        temporary[column * 4 + 2] = b - c;
        temporary[column * 4 + 3] = a - d;
    }

    let mut output = [0_i32; 16];
    for row in 0..4 {
        let dc = temporary[row] + 4;
        let a = dc + temporary[8 + row];
        let b = dc - temporary[8 + row];
        let c = transform_mul2_i32(temporary[4 + row]) - transform_mul1_i32(temporary[12 + row]);
        let d = transform_mul1_i32(temporary[4 + row]) + transform_mul2_i32(temporary[12 + row]);
        output[row * 4] = (a + d) >> 3;
        output[row * 4 + 1] = (b + c) >> 3;
        output[row * 4 + 2] = (b - c) >> 3;
        output[row * 4 + 3] = (a - d) >> 3;
    }
    output
}

/// Performs the VP8 4×4 inverse Walsh-Hadamard transform for Y2 DC values.
#[must_use]
pub fn inverse_wht_4x4(coefficients: [i16; 16]) -> [i32; 16] {
    inverse_wht_4x4_i32(coefficients.map(i32::from))
}

/// Performs VP8's integer inverse Walsh-Hadamard transform on widened Y2 DC
/// coefficients.
#[must_use]
pub fn inverse_wht_4x4_i32(coefficients: [i32; 16]) -> [i32; 16] {
    let mut temporary = [0_i32; 16];
    for column in 0..4 {
        let a0 = coefficients[column] + coefficients[12 + column];
        let a1 = coefficients[4 + column] + coefficients[8 + column];
        let a2 = coefficients[4 + column] - coefficients[8 + column];
        let a3 = coefficients[column] - coefficients[12 + column];
        temporary[column] = a0 + a1;
        temporary[8 + column] = a0 - a1;
        temporary[4 + column] = a3 + a2;
        temporary[12 + column] = a3 - a2;
    }

    let mut output = [0_i32; 16];
    for row in 0..4 {
        let dc = temporary[row * 4] + 3;
        let a0 = dc + temporary[3 + row * 4];
        let a1 = temporary[1 + row * 4] + temporary[2 + row * 4];
        let a2 = temporary[1 + row * 4] - temporary[2 + row * 4];
        let a3 = dc - temporary[3 + row * 4];
        output[row * 4] = (a0 + a1) >> 3;
        output[row * 4 + 1] = (a3 + a2) >> 3;
        output[row * 4 + 2] = (a0 - a1) >> 3;
        output[row * 4 + 3] = (a3 - a2) >> 3;
    }
    output
}

fn transform_mul1_i32(value: i32) -> i32 {
    let result = ((i64::from(value) * 20_091) >> 16) + i64::from(value);
    result.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}

fn transform_mul2_i32(value: i32) -> i32 {
    let result = (i64::from(value) * 35_468) >> 16;
    result.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}

/// Canonical VP8 coefficient probabilities after first-partition updates.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoefficientProbabilities {
    values: [[[[u8; 11]; 3]; 8]; 4],
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

    fn nodes(
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

/// One VP8 intra 4×4 prediction mode.
///
/// Numeric values match VP8's B-mode entropy contexts and therefore can be
/// used directly to index the fixed B_PRED probability table.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Intra4Mode {
    Dc = 0,
    TrueMotion = 1,
    Vertical = 2,
    Horizontal = 3,
    DiagonalDownRight = 4,
    VerticalRight = 5,
    DiagonalDownLeft = 6,
    VerticalLeft = 7,
    HorizontalDown = 8,
    HorizontalUp = 9,
}

/// The luma prediction choice for one VP8 intra macroblock.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LumaMode {
    /// One prediction mode covers the full 16×16 luma macroblock.
    Sixteen(Intra16Mode),
    /// Each luma 4×4 block supplies its own prediction mode in raster order.
    FourByFour([Intra4Mode; 16]),
}

/// One of VP8's four 16×16 luma prediction modes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Intra16Mode {
    Dc,
    Vertical,
    Horizontal,
    TrueMotion,
}

impl Intra16Mode {
    const fn context(self) -> Intra4Mode {
        match self {
            Self::Dc => Intra4Mode::Dc,
            Self::Vertical => Intra4Mode::Vertical,
            Self::Horizontal => Intra4Mode::Horizontal,
            Self::TrueMotion => Intra4Mode::TrueMotion,
        }
    }
}

/// One of VP8's four chroma prediction modes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChromaMode {
    Dc,
    Vertical,
    Horizontal,
    TrueMotion,
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
            top.fill(mode.context());
            left.fill(mode.context());
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

/// Dequantized frequency-domain coefficients for one VP8 macroblock.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DequantizedMacroblock {
    pub luma: [[i32; 16]; 16],
    pub u: [[i32; 16]; 4],
    pub v: [[i32; 16]; 4],
}

/// Spatial-domain signed residues for one VP8 macroblock before prediction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MacroblockSpatialResidues {
    pub luma: [[i32; 16]; 16],
    pub u: [[i32; 16]; 4],
    pub v: [[i32; 16]; 4],
}

/// Reconstructed YUV samples for one VP8 16×16 macroblock.
///
/// Luma is stored as a 16×16 row-major plane; U and V are 8×8 row-major
/// planes, following WebP's mandated 4:2:0 sampling.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MacroblockPixels {
    pub y: [u8; 256],
    pub u: [u8; 64],
    pub v: [u8; 64],
}

/// Already-reconstructed samples adjacent to one macroblock.
///
/// Missing top or left edges model the first macroblock row or column. The
/// top-left samples are consulted only by true-motion prediction, which is
/// invalid at a missing boundary.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct MacroblockPredictionEdges {
    pub top_y: Option<[u8; 16]>,
    /// Four luma samples immediately right of `top_y`, needed by B_PRED.
    pub top_right_y: Option<[u8; 4]>,
    pub left_y: Option<[u8; 16]>,
    pub top_left_y: u8,
    pub top_u: Option<[u8; 8]>,
    pub left_u: Option<[u8; 8]>,
    pub top_left_u: u8,
    pub top_v: Option<[u8; 8]>,
    pub left_v: Option<[u8; 8]>,
    pub top_left_v: u8,
}

/// Applies one segment's VP8 dequantization matrix to a macroblock.
///
/// For a 16×16-predicted luma macroblock, this also inverse-transforms the
/// Y2 block and places its sixteen DC values into the luma blocks, matching
/// VP8's coefficient layout. All output is widened to `i32`.
#[must_use]
pub fn dequantize_macroblock(
    residuals: &MacroblockResiduals,
    matrix: DequantizationMatrix,
) -> DequantizedMacroblock {
    let mut luma = residuals
        .luma
        .map(|block| dequantize_block(block.values, matrix.y1_dc, matrix.y1_ac));
    if let Some(y2) = residuals.y2 {
        let y2_values = dequantize_block(y2.values, matrix.y2_dc, matrix.y2_ac);
        for (block, dc) in luma.iter_mut().zip(inverse_wht_4x4_i32(y2_values)) {
            block[0] = dc;
        }
    }
    DequantizedMacroblock {
        luma,
        u: residuals
            .u
            .map(|block| dequantize_block(block.values, matrix.uv_dc, matrix.uv_ac)),
        v: residuals
            .v
            .map(|block| dequantize_block(block.values, matrix.uv_dc, matrix.uv_ac)),
    }
}

/// Applies VP8's inverse 4×4 DCT to every dequantized macroblock block.
#[must_use]
pub fn inverse_transform_macroblock(
    coefficients: DequantizedMacroblock,
) -> MacroblockSpatialResidues {
    MacroblockSpatialResidues {
        luma: coefficients.luma.map(inverse_dct_4x4_i32),
        u: coefficients.u.map(inverse_dct_4x4_i32),
        v: coefficients.v.map(inverse_dct_4x4_i32),
    }
}

/// Adds inverse-transform residues to a predicted macroblock and clips YUV
/// samples to the valid `0..=255` range.
#[must_use]
pub fn combine_macroblock_prediction(
    mut prediction: MacroblockPixels,
    residues: MacroblockSpatialResidues,
) -> MacroblockPixels {
    combine_plane_blocks(&mut prediction.y, 16, 4, residues.luma);
    combine_plane_blocks(&mut prediction.u, 8, 2, residues.u);
    combine_plane_blocks(&mut prediction.v, 8, 2, residues.v);
    prediction
}

/// Builds a 16×16-luma/8×8-chroma VP8 intra prediction for non-B_PRED luma.
///
/// VP8 initializes unavailable top and left neighbours to its 127 and 129
/// sentinel values, respectively. DC prediction retains its separate edge
/// averaging rules.
#[must_use]
pub fn predict_intra16_macroblock(
    luma_mode: Intra16Mode,
    chroma_mode: ChromaMode,
    edges: MacroblockPredictionEdges,
) -> MacroblockPixels {
    let mut prediction = MacroblockPixels {
        y: [0; 256],
        u: [0; 64],
        v: [0; 64],
    };
    predict_plane(
        &mut prediction.y,
        luma_mode.into(),
        edges.top_y,
        edges.left_y,
        edges.top_left_y,
    );
    predict_plane(
        &mut prediction.u,
        chroma_mode.into(),
        edges.top_u,
        edges.left_u,
        edges.top_left_u,
    );
    predict_plane(
        &mut prediction.v,
        chroma_mode.into(),
        edges.top_v,
        edges.left_v,
        edges.top_left_v,
    );
    prediction
}

/// Builds the luma prediction plane for one VP8 B_PRED macroblock.
///
/// Blocks are predicted in raster order, so every block after the first reads
/// reconstructed samples written by its earlier neighbours. At a picture edge
/// VP8 uses 127 top and 129 left sentinel samples; absent top-right samples
/// replicate the final top sample, matching the rightmost-macroblock rule.
#[must_use]
pub fn predict_intra4_macroblock(
    modes: [Intra4Mode; 16],
    edges: MacroblockPredictionEdges,
) -> [u8; 256] {
    let top_boundary = edges.top_y.unwrap_or([127; 16]);
    let left_boundary = edges.left_y.unwrap_or([129; 16]);
    let top_right = edges.top_right_y.unwrap_or([top_boundary[15]; 4]);
    let top_left = if edges.top_y.is_none() {
        127
    } else if edges.left_y.is_none() {
        129
    } else {
        edges.top_left_y
    };
    let mut output = [0_u8; 256];
    for (block_index, mode) in modes.into_iter().enumerate() {
        let block_x = (block_index % 4) * 4;
        let block_y = (block_index / 4) * 4;
        let top = std::array::from_fn(|index| {
            let x = block_x + index;
            if x >= 16 {
                top_right[x - 16]
            } else if block_y == 0 {
                top_boundary[x]
            } else {
                output[(block_y - 1) * 16 + x]
            }
        });
        let left = std::array::from_fn(|index| {
            let y = block_y + index;
            if block_x == 0 {
                left_boundary[y]
            } else {
                output[y * 16 + block_x - 1]
            }
        });
        let block_top_left = if block_x == 0 {
            if block_y == 0 {
                top_left
            } else {
                left_boundary[block_y - 1]
            }
        } else if block_y == 0 {
            top_boundary[block_x - 1]
        } else {
            output[(block_y - 1) * 16 + block_x - 1]
        };
        let block = predict_intra4_block(mode, block_top_left, top, left);
        for row in 0..4 {
            output[(block_y + row) * 16 + block_x..(block_y + row) * 16 + block_x + 4]
                .copy_from_slice(&block[row * 4..row * 4 + 4]);
        }
    }
    output
}

/// Reconstructs one complete VP8 intra macroblock from entropy tokens.
///
/// This combines segment dequantization, inverse transforms, intra prediction,
/// and sample clipping. Macroblock-row orchestration owns the supplied edge
/// cache and calls this once mode and residual token parsing are complete.
pub fn reconstruct_intra_macroblock(
    block: IntraMacroblock,
    residuals: &MacroblockResiduals,
    matrix: DequantizationMatrix,
    edges: MacroblockPredictionEdges,
) -> Result<MacroblockPixels, DecodeError> {
    let spatial = inverse_transform_macroblock(dequantize_macroblock(residuals, matrix));
    let prediction = match block.luma {
        LumaMode::Sixteen(mode) => predict_intra16_macroblock(mode, block.chroma, edges),
        LumaMode::FourByFour(modes) => {
            let mut prediction = predict_intra16_macroblock(Intra16Mode::Dc, block.chroma, edges);
            prediction.y = predict_intra4_macroblock(modes, edges);
            prediction
        }
    };
    Ok(combine_macroblock_prediction(prediction, spatial))
}

#[derive(Clone, Copy)]
enum PlanePredictionMode {
    Dc,
    Vertical,
    Horizontal,
    TrueMotion,
}

impl From<Intra16Mode> for PlanePredictionMode {
    fn from(mode: Intra16Mode) -> Self {
        match mode {
            Intra16Mode::Dc => Self::Dc,
            Intra16Mode::Vertical => Self::Vertical,
            Intra16Mode::Horizontal => Self::Horizontal,
            Intra16Mode::TrueMotion => Self::TrueMotion,
        }
    }
}

impl From<ChromaMode> for PlanePredictionMode {
    fn from(mode: ChromaMode) -> Self {
        match mode {
            ChromaMode::Dc => Self::Dc,
            ChromaMode::Vertical => Self::Vertical,
            ChromaMode::Horizontal => Self::Horizontal,
            ChromaMode::TrueMotion => Self::TrueMotion,
        }
    }
}

fn predict_plane<const SIZE: usize>(
    output: &mut [u8],
    mode: PlanePredictionMode,
    top: Option<[u8; SIZE]>,
    left: Option<[u8; SIZE]>,
    top_left: u8,
) {
    debug_assert_eq!(output.len(), SIZE * SIZE);
    match mode {
        PlanePredictionMode::Dc => {
            let value = match (top, left) {
                (Some(top), Some(left)) => {
                    let sum = top.into_iter().map(u32::from).sum::<u32>()
                        + left.into_iter().map(u32::from).sum::<u32>();
                    ((sum + SIZE as u32) / (2 * SIZE) as u32) as u8
                }
                (Some(top), None) => {
                    ((top.into_iter().map(u32::from).sum::<u32>() + (SIZE / 2) as u32)
                        / SIZE as u32) as u8
                }
                (None, Some(left)) => {
                    ((left.into_iter().map(u32::from).sum::<u32>() + (SIZE / 2) as u32)
                        / SIZE as u32) as u8
                }
                (None, None) => 128,
            };
            output.fill(value);
        }
        PlanePredictionMode::Vertical => {
            let top = top.unwrap_or([127; SIZE]);
            for row in output.chunks_exact_mut(SIZE) {
                row.copy_from_slice(&top);
            }
        }
        PlanePredictionMode::Horizontal => {
            let left = left.unwrap_or([129; SIZE]);
            for (row, &value) in output.chunks_exact_mut(SIZE).zip(left.iter()) {
                row.fill(value);
            }
        }
        PlanePredictionMode::TrueMotion => {
            let top_left = match (top, left) {
                (None, _) => 127,
                (Some(_), None) => 129,
                (Some(_), Some(_)) => top_left,
            };
            let top = top.unwrap_or([127; SIZE]);
            let left = left.unwrap_or([129; SIZE]);
            for (row, &left_value) in output.chunks_exact_mut(SIZE).zip(left.iter()) {
                for (sample, &top_value) in row.iter_mut().zip(top.iter()) {
                    *sample = (i32::from(left_value) + i32::from(top_value) - i32::from(top_left))
                        .clamp(0, 255) as u8;
                }
            }
        }
    }
}

/// Predicts one VP8 B_PRED luma 4×4 block from its already-reconstructed
/// neighbours. `top` supplies the four direct and four top-right samples.
#[must_use]
pub fn predict_intra4_block(
    mode: Intra4Mode,
    top_left: u8,
    top: [u8; 8],
    left: [u8; 4],
) -> [u8; 16] {
    let mut out = [0_u8; 16];
    let set = |out: &mut [u8; 16], x: usize, y: usize, value: u8| out[y * 4 + x] = value;
    let a2 = |a: u8, b: u8| ((u16::from(a) + u16::from(b) + 1) >> 1) as u8;
    let a3 =
        |a: u8, b: u8, c: u8| ((u16::from(a) + 2 * u16::from(b) + u16::from(c) + 2) >> 2) as u8;
    match mode {
        Intra4Mode::Dc => {
            let value = (top[..4]
                .iter()
                .chain(left.iter())
                .map(|&value| u16::from(value))
                .sum::<u16>()
                + 4)
                >> 3;
            out.fill(value as u8);
        }
        Intra4Mode::TrueMotion => {
            for (y, &left_value) in left.iter().enumerate() {
                for (x, &top_value) in top[..4].iter().enumerate() {
                    set(
                        &mut out,
                        x,
                        y,
                        (i32::from(left_value) + i32::from(top_value) - i32::from(top_left))
                            .clamp(0, 255) as u8,
                    );
                }
            }
        }
        Intra4Mode::Vertical => {
            let row = [
                a3(top_left, top[0], top[1]),
                a3(top[0], top[1], top[2]),
                a3(top[1], top[2], top[3]),
                a3(top[2], top[3], top[4]),
            ];
            for y in 0..4 {
                out[y * 4..y * 4 + 4].copy_from_slice(&row);
            }
        }
        Intra4Mode::Horizontal => {
            let rows = [
                a3(top_left, left[0], left[1]),
                a3(left[0], left[1], left[2]),
                a3(left[1], left[2], left[3]),
                a3(left[2], left[3], left[3]),
            ];
            for (y, value) in rows.into_iter().enumerate() {
                out[y * 4..y * 4 + 4].fill(value);
            }
        }
        Intra4Mode::DiagonalDownRight => {
            set(&mut out, 0, 3, a3(left[1], left[2], left[3]));
            for (x, y) in [(1, 3), (0, 2)] {
                set(&mut out, x, y, a3(left[0], left[1], left[2]));
            }
            for (x, y) in [(2, 3), (1, 2), (0, 1)] {
                set(&mut out, x, y, a3(top_left, left[0], left[1]));
            }
            for (x, y) in [(3, 3), (2, 2), (1, 1), (0, 0)] {
                set(&mut out, x, y, a3(top[0], top_left, left[0]));
            }
            for (x, y) in [(3, 2), (2, 1), (1, 0)] {
                set(&mut out, x, y, a3(top[1], top[0], top_left));
            }
            for (x, y) in [(3, 1), (2, 0)] {
                set(&mut out, x, y, a3(top[2], top[1], top[0]));
            }
            set(&mut out, 3, 0, a3(top[3], top[2], top[1]));
        }
        Intra4Mode::DiagonalDownLeft => {
            set(&mut out, 0, 0, a3(top[0], top[1], top[2]));
            for (x, y) in [(1, 0), (0, 1)] {
                set(&mut out, x, y, a3(top[1], top[2], top[3]));
            }
            for (x, y) in [(2, 0), (1, 1), (0, 2)] {
                set(&mut out, x, y, a3(top[2], top[3], top[4]));
            }
            for (x, y) in [(3, 0), (2, 1), (1, 2), (0, 3)] {
                set(&mut out, x, y, a3(top[3], top[4], top[5]));
            }
            for (x, y) in [(3, 1), (2, 2), (1, 3)] {
                set(&mut out, x, y, a3(top[4], top[5], top[6]));
            }
            for (x, y) in [(3, 2), (2, 3)] {
                set(&mut out, x, y, a3(top[5], top[6], top[7]));
            }
            set(&mut out, 3, 3, a3(top[6], top[7], top[7]));
        }
        Intra4Mode::VerticalRight => {
            for (x, value) in [
                a2(top_left, top[0]),
                a2(top[0], top[1]),
                a2(top[1], top[2]),
                a2(top[2], top[3]),
            ]
            .into_iter()
            .enumerate()
            {
                set(&mut out, x, 0, value);
            }
            set(&mut out, 0, 3, a3(left[2], left[1], left[0]));
            set(&mut out, 0, 2, a3(left[1], left[0], top_left));
            for (x, y) in [(0, 1), (1, 3)] {
                set(&mut out, x, y, a3(left[0], top_left, top[0]));
            }
            for (x, y) in [(1, 1), (2, 3)] {
                set(&mut out, x, y, a3(top_left, top[0], top[1]));
            }
            for (x, y) in [(2, 1), (3, 3)] {
                set(&mut out, x, y, a3(top[0], top[1], top[2]));
            }
            set(&mut out, 3, 1, a3(top[1], top[2], top[3]));
            for (x, y, value) in [
                (1, 2, a2(top_left, top[0])),
                (2, 2, a2(top[0], top[1])),
                (3, 2, a2(top[1], top[2])),
            ] {
                set(&mut out, x, y, value);
            }
        }
        Intra4Mode::VerticalLeft => {
            for (x, value) in [
                a2(top[0], top[1]),
                a2(top[1], top[2]),
                a2(top[2], top[3]),
                a2(top[3], top[4]),
            ]
            .into_iter()
            .enumerate()
            {
                set(&mut out, x, 0, value);
            }
            for (x, y, value) in [
                (0, 2, a2(top[1], top[2])),
                (1, 2, a2(top[2], top[3])),
                (2, 2, a2(top[3], top[4])),
            ] {
                set(&mut out, x, y, value);
            }
            set(&mut out, 0, 1, a3(top[0], top[1], top[2]));
            for (x, y) in [(1, 1), (0, 3)] {
                set(&mut out, x, y, a3(top[1], top[2], top[3]));
            }
            for (x, y) in [(2, 1), (1, 3)] {
                set(&mut out, x, y, a3(top[2], top[3], top[4]));
            }
            for (x, y) in [(3, 1), (2, 3)] {
                set(&mut out, x, y, a3(top[3], top[4], top[5]));
            }
            set(&mut out, 3, 2, a3(top[4], top[5], top[6]));
            set(&mut out, 3, 3, a3(top[5], top[6], top[7]));
        }
        Intra4Mode::HorizontalUp => {
            set(&mut out, 0, 0, a2(left[0], left[1]));
            for (x, y) in [(2, 0), (0, 1)] {
                set(&mut out, x, y, a2(left[1], left[2]));
            }
            for (x, y) in [(2, 1), (0, 2)] {
                set(&mut out, x, y, a2(left[2], left[3]));
            }
            set(&mut out, 1, 0, a3(left[0], left[1], left[2]));
            for (x, y) in [(3, 0), (1, 1)] {
                set(&mut out, x, y, a3(left[1], left[2], left[3]));
            }
            for (x, y) in [(3, 1), (1, 2)] {
                set(&mut out, x, y, a3(left[2], left[3], left[3]));
            }
            for (x, y) in [(3, 2), (2, 2), (0, 3), (1, 3), (2, 3), (3, 3)] {
                set(&mut out, x, y, left[3]);
            }
        }
        Intra4Mode::HorizontalDown => {
            for (x, y) in [(0, 0), (2, 1)] {
                set(&mut out, x, y, a2(left[0], top_left));
            }
            for (x, y) in [(0, 1), (2, 2)] {
                set(&mut out, x, y, a2(left[1], left[0]));
            }
            for (x, y) in [(0, 2), (2, 3)] {
                set(&mut out, x, y, a2(left[2], left[1]));
            }
            set(&mut out, 0, 3, a2(left[3], left[2]));
            set(&mut out, 3, 0, a3(top[0], top[1], top[2]));
            set(&mut out, 2, 0, a3(top_left, top[0], top[1]));
            for (x, y) in [(1, 0), (3, 1)] {
                set(&mut out, x, y, a3(left[0], top_left, top[0]));
            }
            for (x, y) in [(1, 1), (3, 2)] {
                set(&mut out, x, y, a3(left[1], left[0], top_left));
            }
            for (x, y) in [(1, 2), (3, 3)] {
                set(&mut out, x, y, a3(left[2], left[1], left[0]));
            }
            set(&mut out, 1, 3, a3(left[3], left[2], left[1]));
        }
    }
    out
}

fn combine_plane_blocks<const PIXELS: usize, const BLOCKS: usize>(
    plane: &mut [u8; PIXELS],
    stride: usize,
    blocks_per_row: usize,
    blocks: [[i32; 16]; BLOCKS],
) {
    for (block_index, block) in blocks.into_iter().enumerate() {
        let block_x = (block_index % blocks_per_row) * 4;
        let block_y = (block_index / blocks_per_row) * 4;
        for row in 0..4 {
            for column in 0..4 {
                let destination = (block_y + row) * stride + block_x + column;
                plane[destination] =
                    add_residue_and_clip(plane[destination], block[row * 4 + column]);
            }
        }
    }
}

/// Adds one signed VP8 residue to a prediction sample with saturating clip.
#[must_use]
pub fn add_residue_and_clip(prediction: u8, residue: i32) -> u8 {
    (i32::from(prediction) + residue).clamp(0, 255) as u8
}

fn dequantize_block(values: [i16; 16], dc: u16, ac: u16) -> [i32; 16] {
    let mut output = values.map(|value| i32::from(value) * i32::from(ac));
    output[0] = i32::from(values[0]) * i32::from(dc);
    output
}

/// The parsed prefix of a VP8 first partition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FirstPartitionHeader {
    /// `false` means the WebP-mandated YUV 4:2:0 colour space; `true` is the
    /// VP8 reserved value, retained here for later strict/profile handling.
    pub colorspace_reserved: bool,
    pub clamp_type: bool,
    pub segments: SegmentHeader,
    pub filter: FilterHeader,
    /// Number of coefficient-token partitions: always 1, 2, 4, or 8.
    pub token_partition_count: u8,
    pub quantization: QuantizationHeader,
    /// VP8's `refresh_entropy_probs` bit. Key frames have no prior state to
    /// retain, but this remains observable for future inter-frame support.
    pub refresh_entropy_probabilities: bool,
    pub coefficients: CoefficientProbabilities,
}

/// One coefficient-token partition after its three-byte size table.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TokenPartition<'a> {
    pub data: &'a [u8],
    /// Byte offset from the start of the VP8 RIFF payload.
    pub offset: usize,
}

/// Validated first-partition controls and coefficient-token partition layout.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PartitionLayout<'a> {
    pub header: FirstPartitionHeader,
    pub tokens: Vec<TokenPartition<'a>>,
}

/// Parses the VP8 first-partition prefix and safely partitions token data.
///
/// The supplied [`Vp8Header`] must have been parsed from the same payload. No
/// coefficient, macroblock, or pixel buffer is allocated here. The returned
/// token slices are zero-copy and their offsets are relative to `payload`.
pub fn parse_partition_layout<'a>(
    payload: &'a [u8],
    frame: &Vp8Header,
    limits: &DecodeLimits,
) -> Result<PartitionLayout<'a>, DecodeError> {
    let (layout, _) = parse_partition_layout_with_mode_decoder(payload, frame, limits)?;
    Ok(layout)
}

/// Parses the first partition and returns its decoder at the first mode bit.
///
/// VP8 arithmetic decoding is stateful across the first-partition header and
/// macroblock mode stream, so frame reconstruction must continue this exact
/// decoder rather than create one at a later byte offset.
fn parse_partition_layout_with_mode_decoder<'a>(
    payload: &'a [u8],
    frame: &Vp8Header,
    limits: &DecodeLimits,
) -> Result<(PartitionLayout<'a>, BoolDecoder<'a>), DecodeError> {
    limits.check_input_len(payload.len())?;
    let first_partition_end = KEY_FRAME_HEADER_LEN
        .checked_add(frame.first_partition_len)
        .ok_or_else(|| {
            DecodeError::at(
                DecodeErrorKind::InvalidBitstream,
                FRAME_TAG_LEN,
                "VP8 first partition end overflows",
            )
        })?;
    if first_partition_end > payload.len() {
        return Err(DecodeError::at(
            DecodeErrorKind::UnexpectedEof,
            FRAME_TAG_LEN,
            "VP8 first partition is outside payload",
        ));
    }

    let mut bits = BoolDecoder::new(&payload[KEY_FRAME_HEADER_LEN..first_partition_end], limits)?;
    let colorspace_reserved = bits.read_bool(128)?;
    let clamp_type = bits.read_bool(128)?;
    let segments = parse_segment_header(&mut bits)?;
    let filter = parse_filter_header(&mut bits)?;
    let token_partition_count = 1_u8 << bits.read_literal(2)?;
    let quantization = parse_quantization_header(&mut bits)?;
    let refresh_entropy_probabilities = bits.read_bool(128)?;
    let coefficients = parse_coefficient_probabilities(&mut bits)?;

    let size_table_len = 3_usize * (usize::from(token_partition_count) - 1);
    let token_data_start = first_partition_end
        .checked_add(size_table_len)
        .ok_or_else(|| {
            DecodeError::at(
                DecodeErrorKind::InvalidBitstream,
                first_partition_end,
                "VP8 token-partition table end overflows",
            )
        })?;
    if token_data_start >= payload.len() {
        return Err(DecodeError::at(
            DecodeErrorKind::UnexpectedEof,
            first_partition_end,
            "truncated VP8 token-partition size table or final partition",
        ));
    }

    let mut tokens = Vec::with_capacity(usize::from(token_partition_count));
    let mut table_offset = first_partition_end;
    let mut data_offset = token_data_start;
    for _ in 1..token_partition_count {
        let size = usize::from(payload[table_offset])
            | (usize::from(payload[table_offset + 1]) << 8)
            | (usize::from(payload[table_offset + 2]) << 16);
        table_offset += 3;
        let end = data_offset.checked_add(size).ok_or_else(|| {
            DecodeError::at(
                DecodeErrorKind::InvalidBitstream,
                data_offset,
                "VP8 token partition end overflows",
            )
        })?;
        if end > payload.len() {
            return Err(DecodeError::at(
                DecodeErrorKind::UnexpectedEof,
                data_offset,
                "VP8 token partition exceeds payload",
            ));
        }
        tokens.push(TokenPartition {
            data: &payload[data_offset..end],
            offset: data_offset,
        });
        data_offset = end;
    }
    tokens.push(TokenPartition {
        data: &payload[data_offset..],
        offset: data_offset,
    });

    Ok((
        PartitionLayout {
            header: FirstPartitionHeader {
                colorspace_reserved,
                clamp_type,
                segments,
                filter,
                token_partition_count,
                quantization,
                refresh_entropy_probabilities,
                coefficients,
            },
            tokens,
        },
        bits,
    ))
}

fn parse_segment_header(bits: &mut BoolDecoder<'_>) -> Result<SegmentHeader, DecodeError> {
    let enabled = bits.read_bool(128)?;
    if !enabled {
        return Ok(SegmentHeader {
            enabled: false,
            update_map: false,
            absolute_delta: true,
            quantizer: [0; 4],
            filter_strength: [0; 4],
            probabilities: [255; 3],
        });
    }
    let update_map = bits.read_bool(128)?;
    let mut absolute_delta = true;
    let mut quantizer = [0; 4];
    let mut filter_strength = [0; 4];
    if bits.read_bool(128)? {
        absolute_delta = bits.read_bool(128)?;
        for value in &mut quantizer {
            if bits.read_bool(128)? {
                *value = bits.read_signed_literal(7)?;
            }
        }
        for value in &mut filter_strength {
            if bits.read_bool(128)? {
                *value = bits.read_signed_literal(6)?;
            }
        }
    }
    let mut probabilities = [255; 3];
    if update_map {
        for value in &mut probabilities {
            if bits.read_bool(128)? {
                *value = bits.read_literal(8)? as u8;
            }
        }
    }
    Ok(SegmentHeader {
        enabled,
        update_map,
        absolute_delta,
        quantizer,
        filter_strength,
        probabilities,
    })
}

fn parse_filter_header(bits: &mut BoolDecoder<'_>) -> Result<FilterHeader, DecodeError> {
    let simple = bits.read_bool(128)?;
    let level = bits.read_literal(6)? as u8;
    let sharpness = bits.read_literal(3)? as u8;
    let use_deltas = bits.read_bool(128)?;
    let mut ref_deltas = [0; 4];
    let mut mode_deltas = [0; 4];
    if use_deltas && bits.read_bool(128)? {
        for value in &mut ref_deltas {
            if bits.read_bool(128)? {
                *value = bits.read_signed_literal(6)?;
            }
        }
        for value in &mut mode_deltas {
            if bits.read_bool(128)? {
                *value = bits.read_signed_literal(6)?;
            }
        }
    }
    Ok(FilterHeader {
        simple,
        level,
        sharpness,
        use_deltas,
        ref_deltas,
        mode_deltas,
    })
}

fn parse_quantization_header(
    bits: &mut BoolDecoder<'_>,
) -> Result<QuantizationHeader, DecodeError> {
    let base_index = bits.read_literal(7)? as u8;
    let mut deltas = [0; 5];
    for value in &mut deltas {
        if bits.read_bool(128)? {
            *value = bits.read_signed_literal(4)?;
        }
    }
    Ok(QuantizationHeader {
        base_index,
        y1_dc_delta: deltas[0],
        y2_dc_delta: deltas[1],
        y2_ac_delta: deltas[2],
        uv_dc_delta: deltas[3],
        uv_ac_delta: deltas[4],
    })
}

fn parse_coefficient_probabilities(
    bits: &mut BoolDecoder<'_>,
) -> Result<CoefficientProbabilities, DecodeError> {
    let mut probabilities = CoefficientProbabilities::default();
    for (coefficient_type, bands) in COEFFICIENT_UPDATE_PROBABILITIES.iter().enumerate() {
        for (band, contexts) in bands.iter().enumerate() {
            for (context, nodes) in contexts.iter().enumerate() {
                for (node, &update_probability) in nodes.iter().enumerate() {
                    if bits.read_bool(update_probability)? {
                        probabilities.values[coefficient_type][band][context][node] =
                            bits.read_literal(8)? as u8;
                    }
                }
            }
        }
    }
    probabilities.use_skip_probability = bits.read_bool(128)?;
    if probabilities.use_skip_probability {
        probabilities.skip_probability = bits.read_literal(8)? as u8;
    }
    Ok(probabilities)
}

/// Parsed VP8 frame tag and the fixed portion of a key-frame header.
///
/// `first_partition_len` counts bytes after the key frame's fixed ten-byte
/// header. Its end is validated against the supplied VP8 payload but its
/// compressed header is parsed by the entropy stage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Vp8Header {
    pub width: u32,
    pub height: u32,
    pub version: u8,
    pub first_partition_len: usize,
    pub horizontal_scale: u8,
    pub vertical_scale: u8,
}

/// Parses a VP8 payload from a `VP8 ` RIFF chunk.
///
/// WebP still images must be visible VP8 key frames using one of versions
/// `0..=3`.  The parser rejects malformed or incomplete headers and applies
/// image limits before any macroblock allocation is possible.
pub fn parse_riff_payload(
    payload: &[u8],
    canvas: Option<(u32, u32)>,
    limits: &DecodeLimits,
) -> Result<Vp8Header, DecodeError> {
    limits.check_input_len(payload.len())?;
    if payload.len() < KEY_FRAME_HEADER_LEN {
        return Err(DecodeError::at(
            DecodeErrorKind::UnexpectedEof,
            payload.len(),
            "truncated VP8 key-frame header",
        ));
    }

    let frame_tag =
        u32::from(payload[0]) | (u32::from(payload[1]) << 8) | (u32::from(payload[2]) << 16);
    if frame_tag & 1 != 0 {
        return Err(DecodeError::at(
            DecodeErrorKind::UnsupportedFeature,
            0,
            "inter frames are not valid WebP still-image payloads",
        ));
    }
    let version = ((frame_tag >> 1) & 0x7) as u8;
    if version > 3 {
        return Err(DecodeError::at(
            DecodeErrorKind::InvalidBitstream,
            0,
            "VP8 frame version is outside the WebP-supported range",
        ));
    }
    if frame_tag & (1 << 4) == 0 {
        return Err(DecodeError::at(
            DecodeErrorKind::InvalidBitstream,
            0,
            "WebP still-image key frame must be visible",
        ));
    }

    let first_partition_len = usize::try_from(frame_tag >> 5).map_err(|_| {
        DecodeError::at(
            DecodeErrorKind::InvalidBitstream,
            0,
            "VP8 first partition length does not fit usize",
        )
    })?;
    if first_partition_len > payload.len() - KEY_FRAME_HEADER_LEN {
        return Err(DecodeError::at(
            DecodeErrorKind::UnexpectedEof,
            FRAME_TAG_LEN,
            "VP8 first partition exceeds payload",
        ));
    }
    if payload[3..6] != KEY_FRAME_START_CODE {
        return Err(DecodeError::at(
            DecodeErrorKind::InvalidBitstream,
            FRAME_TAG_LEN,
            "invalid VP8 key-frame start code",
        ));
    }

    let width_field = u16::from_le_bytes([payload[6], payload[7]]);
    let height_field = u16::from_le_bytes([payload[8], payload[9]]);
    let width = u32::from(width_field & 0x3fff);
    let height = u32::from(height_field & 0x3fff);
    if width == 0 || height == 0 {
        return Err(DecodeError::at(
            DecodeErrorKind::InvalidBitstream,
            6,
            "VP8 image dimensions must be non-zero",
        ));
    }
    limits.check_image(width, height)?;
    if let Some((canvas_width, canvas_height)) = canvas
        && (canvas_width != width || canvas_height != height)
    {
        return Err(DecodeError::at(
            DecodeErrorKind::InvalidContainer,
            0,
            "VP8 dimensions do not match VP8X canvas",
        ));
    }

    Ok(Vp8Header {
        width,
        height,
        version,
        first_partition_len,
        horizontal_scale: ((width_field >> 14) & 0x3) as u8,
        vertical_scale: ((height_field >> 14) & 0x3) as u8,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loop_filter::{MacroblockFilter, filter_macroblock};

    /// A deliberately straightforward VP8 boolean writer used only to produce
    /// independently driven decoder vectors. It follows the encoder interval
    /// update and byte-flush rules, not the decoder's cached-value structure.
    #[derive(Default)]
    struct TestBoolWriter {
        range: i32,
        value: i32,
        run: usize,
        pending_bits: i32,
        bytes: Vec<u8>,
    }

    impl TestBoolWriter {
        fn new() -> Self {
            Self {
                range: 254,
                value: 0,
                run: 0,
                pending_bits: -8,
                bytes: Vec::new(),
            }
        }

        fn write_bool(&mut self, bit: bool, probability: u8) {
            let split = (self.range * i32::from(probability)) >> 8;
            if bit {
                self.value += split + 1;
                self.range -= split + 1;
            } else {
                self.range = split;
            }
            if self.range < 127 {
                let shift = if self.range == 0 {
                    7
                } else {
                    7 - self.range.ilog2() as i32
                };
                self.range = ((self.range + 1) << shift) - 1;
                self.value <<= shift;
                self.pending_bits += shift;
                if self.pending_bits > 0 {
                    self.flush();
                }
            }
        }

        fn write_literal(&mut self, value: u32, count: u8) {
            for shift in (0..count).rev() {
                self.write_bool(((value >> shift) & 1) != 0, 128);
            }
        }

        fn write_signed_literal(&mut self, value: i32, count: u8) {
            self.write_literal(value.unsigned_abs(), count);
            self.write_bool(value.is_negative(), 128);
        }

        fn finish(mut self) -> Vec<u8> {
            self.write_literal(0, (9 - self.pending_bits) as u8);
            self.pending_bits = 0;
            self.flush();
            self.bytes
        }

        fn flush(&mut self) {
            let shift = 8 + self.pending_bits;
            let bits = self.value >> shift;
            self.value -= bits << shift;
            self.pending_bits -= 8;
            if bits & 0xff == 0xff {
                self.run += 1;
                return;
            }
            if bits & 0x100 != 0
                && let Some(previous) = self.bytes.last_mut()
            {
                *previous += 1;
            }
            let delayed = if bits & 0x100 != 0 { 0 } else { 0xff };
            self.bytes.extend(std::iter::repeat_n(delayed, self.run));
            self.run = 0;
            self.bytes.push((bits & 0xff) as u8);
        }
    }

    fn write_quantization_header(
        writer: &mut TestBoolWriter,
        base_index: u8,
        deltas: [i32; 5],
        refresh_entropy_probabilities: bool,
    ) {
        writer.write_literal(u32::from(base_index), 7);
        for value in deltas {
            writer.write_bool(value != 0, 128);
            if value != 0 {
                writer.write_signed_literal(value, 4);
            }
        }
        writer.write_bool(refresh_entropy_probabilities, 128);
    }

    fn write_coefficient_updates(
        writer: &mut TestBoolWriter,
        updates: &[(usize, usize, usize, usize, u8)],
        use_skip_probability: bool,
        skip_probability: u8,
    ) {
        for (coefficient_type, bands) in COEFFICIENT_UPDATE_PROBABILITIES.iter().enumerate() {
            for (band, contexts) in bands.iter().enumerate() {
                for (context, nodes) in contexts.iter().enumerate() {
                    for (node, &update_probability) in nodes.iter().enumerate() {
                        let update = updates.iter().find(|&&(t, b, c, n, _)| {
                            (t, b, c, n) == (coefficient_type, band, context, node)
                        });
                        writer.write_bool(update.is_some(), update_probability);
                        if let Some(&(_, _, _, _, value)) = update {
                            writer.write_literal(u32::from(value), 8);
                        }
                    }
                }
            }
        }
        writer.write_bool(use_skip_probability, 128);
        if use_skip_probability {
            writer.write_literal(u32::from(skip_probability), 8);
        }
    }

    fn pad_first_partition(writer: &mut TestBoolWriter) {
        writer.write_literal(0, 8); // Leave structural fields away from EOF.
    }

    fn coefficient_nodes(
        probabilities: &CoefficientProbabilities,
        coefficient_type: CoefficientBlockType,
        position: usize,
        context: usize,
    ) -> &[u8; 11] {
        probabilities.nodes(coefficient_type, position, context)
    }

    fn first_partition_header(
        segments: SegmentHeader,
        coefficients: CoefficientProbabilities,
    ) -> FirstPartitionHeader {
        FirstPartitionHeader {
            colorspace_reserved: false,
            clamp_type: false,
            segments,
            filter: FilterHeader {
                simple: false,
                level: 0,
                sharpness: 0,
                use_deltas: false,
                ref_deltas: [0; 4],
                mode_deltas: [0; 4],
            },
            token_partition_count: 1,
            quantization: QuantizationHeader {
                base_index: 0,
                y1_dc_delta: 0,
                y2_dc_delta: 0,
                y2_ac_delta: 0,
                uv_dc_delta: 0,
                uv_ac_delta: 0,
            },
            refresh_entropy_probabilities: false,
            coefficients,
        }
    }

    fn write_coefficient_eob(
        writer: &mut TestBoolWriter,
        probabilities: &CoefficientProbabilities,
        coefficient_type: CoefficientBlockType,
        position: usize,
        context: usize,
    ) {
        writer.write_bool(
            false,
            coefficient_nodes(probabilities, coefficient_type, position, context)[0],
        );
    }

    fn key_frame(
        width: u16,
        height: u16,
        version: u8,
        show_frame: bool,
        partition_len: u32,
    ) -> [u8; KEY_FRAME_HEADER_LEN] {
        let tag = (partition_len << 5) | (u32::from(show_frame) << 4) | (u32::from(version) << 1);
        let mut payload = [0_u8; KEY_FRAME_HEADER_LEN];
        payload[..3].copy_from_slice(&tag.to_le_bytes()[..3]);
        payload[3..6].copy_from_slice(&KEY_FRAME_START_CODE);
        payload[6..8].copy_from_slice(&width.to_le_bytes());
        payload[8..10].copy_from_slice(&height.to_le_bytes());
        payload
    }

    #[test]
    fn parses_key_frame_dimensions_tag_and_scale_bits() {
        let payload = key_frame(0x800d, 0xc009, 3, true, 0);
        let header = parse_riff_payload(&payload, Some((13, 9)), &DecodeLimits::default()).unwrap();
        assert_eq!(header.width, 13);
        assert_eq!(header.height, 9);
        assert_eq!(header.version, 3);
        assert_eq!(header.first_partition_len, 0);
        assert_eq!(header.horizontal_scale, 2);
        assert_eq!(header.vertical_scale, 3);
    }

    #[test]
    fn rejects_all_fixed_header_truncations() {
        let payload = key_frame(1, 1, 0, true, 0);
        for end in 0..KEY_FRAME_HEADER_LEN {
            assert_eq!(
                parse_riff_payload(&payload[..end], None, &DecodeLimits::default())
                    .unwrap_err()
                    .kind(),
                DecodeErrorKind::UnexpectedEof,
                "truncation at {end}",
            );
        }
    }

    #[test]
    fn rejects_invalid_tag_signature_dimensions_partition_and_canvas() {
        let limits = DecodeLimits::default();
        let mut inter = key_frame(1, 1, 0, true, 0);
        inter[0] |= 1;
        assert_eq!(
            parse_riff_payload(&inter, None, &limits)
                .unwrap_err()
                .kind(),
            DecodeErrorKind::UnsupportedFeature
        );

        let invisible = key_frame(1, 1, 0, false, 0);
        assert_eq!(
            parse_riff_payload(&invisible, None, &limits)
                .unwrap_err()
                .kind(),
            DecodeErrorKind::InvalidBitstream
        );

        let unsupported_version = key_frame(1, 1, 4, true, 0);
        assert_eq!(
            parse_riff_payload(&unsupported_version, None, &limits)
                .unwrap_err()
                .kind(),
            DecodeErrorKind::InvalidBitstream
        );

        let mut bad_signature = key_frame(1, 1, 0, true, 0);
        bad_signature[5] ^= 1;
        assert_eq!(
            parse_riff_payload(&bad_signature, None, &limits)
                .unwrap_err()
                .kind(),
            DecodeErrorKind::InvalidBitstream
        );

        let zero_width = key_frame(0, 1, 0, true, 0);
        assert_eq!(
            parse_riff_payload(&zero_width, None, &limits)
                .unwrap_err()
                .kind(),
            DecodeErrorKind::InvalidBitstream
        );

        let partition_past_end = key_frame(1, 1, 0, true, 1);
        assert_eq!(
            parse_riff_payload(&partition_past_end, None, &limits)
                .unwrap_err()
                .kind(),
            DecodeErrorKind::UnexpectedEof
        );

        let valid = key_frame(1, 1, 0, true, 0);
        assert_eq!(
            parse_riff_payload(&valid, Some((2, 1)), &limits)
                .unwrap_err()
                .kind(),
            DecodeErrorKind::InvalidContainer
        );
    }

    #[test]
    fn enforces_image_limits_before_decoder_state_is_created() {
        let payload = key_frame(8, 1, 0, true, 0);
        let limits = DecodeLimits {
            max_width: 7,
            ..DecodeLimits::default()
        };
        assert_eq!(
            parse_riff_payload(&payload, None, &limits)
                .unwrap_err()
                .kind(),
            DecodeErrorKind::LimitExceeded
        );
    }

    #[test]
    fn boolean_decoder_recovers_mixed_probability_vectors() {
        let probabilities = [1_u8, 2, 127, 128, 254, 1, 128, 254, 2];
        let expected = [true, false, true, true, false, false, true, true, false];
        let mut writer = TestBoolWriter::new();
        for (&bit, &probability) in expected.iter().zip(probabilities.iter()) {
            writer.write_bool(bit, probability);
        }
        let bytes = writer.finish();
        assert!(!bytes.is_empty());

        let mut decoder = BoolDecoder::new(&bytes, &DecodeLimits::default()).unwrap();
        for (index, (&bit, &probability)) in expected.iter().zip(probabilities.iter()).enumerate() {
            assert_eq!(
                decoder.read_bool(probability).unwrap(),
                bit,
                "symbol {index}"
            );
        }
        assert_eq!(
            decoder.remaining_work(),
            DecodeLimits::default().max_work_units - expected.len() as u64
        );
        assert!(decoder.bytes_consumed() <= bytes.len());
    }

    #[test]
    fn boolean_decoder_handles_extreme_probabilities() {
        let mut true_values = BoolDecoder::new(&[0xff], &DecodeLimits::default()).unwrap();
        assert_eq!(true_values.read_bool(0), Ok(true));
        assert_eq!(true_values.read_bool(255), Ok(true));

        let mut false_value = BoolDecoder::new(&[0], &DecodeLimits::default()).unwrap();
        assert_eq!(false_value.read_bool(255), Ok(false));
    }

    #[test]
    fn boolean_decoder_reads_msb_first_literals() {
        let mut writer = TestBoolWriter::new();
        writer.write_literal(0b10110, 5);
        writer.write_literal(0x1234, 16);
        writer.write_signed_literal(-17, 7);
        writer.write_bool(false, 128); // Keep the signed value away from EOF.
        let bytes = writer.finish();
        let mut decoder = BoolDecoder::new(&bytes, &DecodeLimits::default()).unwrap();
        assert_eq!(decoder.read_literal(5), Ok(0b10110));
        assert_eq!(decoder.read_literal(16), Ok(0x1234));
        assert_eq!(decoder.read_signed_literal(7), Ok(-17));
        assert_eq!(
            decoder.read_literal(33).unwrap_err().kind(),
            DecodeErrorKind::InvalidParameter
        );
    }

    #[test]
    fn boolean_decoder_reports_eof_and_work_budget_exhaustion() {
        let mut empty = BoolDecoder::new(&[], &DecodeLimits::default()).unwrap();
        assert_eq!(
            empty.read_bool(128).unwrap_err().kind(),
            DecodeErrorKind::UnexpectedEof
        );

        let limited = DecodeLimits {
            max_work_units: 1,
            ..DecodeLimits::default()
        };
        let mut decoder = BoolDecoder::new(&[0], &limited).unwrap();
        assert!(decoder.read_bool(128).is_ok());
        assert_eq!(
            decoder.read_bool(128).unwrap_err().kind(),
            DecodeErrorKind::LimitExceeded
        );
    }

    #[test]
    fn parses_first_partition_controls_and_four_token_partitions() {
        let mut writer = TestBoolWriter::new();
        writer.write_bool(false, 128); // colour space
        writer.write_bool(true, 128); // clamp type
        writer.write_bool(true, 128); // segmentation enabled
        writer.write_bool(true, 128); // update segment map
        writer.write_bool(true, 128); // update segment data
        writer.write_bool(false, 128); // delta rather than absolute values
        for value in [-5, 0, 3, 0] {
            writer.write_bool(value != 0, 128);
            if value != 0 {
                writer.write_signed_literal(value, 7);
            }
        }
        for value in [-4, 0, 0, 0] {
            writer.write_bool(value != 0, 128);
            if value != 0 {
                writer.write_signed_literal(value, 6);
            }
        }
        for value in [11_u8, 255, 77] {
            writer.write_bool(value != 255, 128);
            if value != 255 {
                writer.write_literal(u32::from(value), 8);
            }
        }
        writer.write_bool(false, 128); // normal filter
        writer.write_literal(17, 6);
        writer.write_literal(4, 3);
        writer.write_bool(true, 128); // loop-filter deltas enabled
        writer.write_bool(true, 128); // update deltas
        for value in [2, 0, 0, 0] {
            writer.write_bool(value != 0, 128);
            if value != 0 {
                writer.write_signed_literal(value, 6);
            }
        }
        for value in [0, 0, 0, -1] {
            writer.write_bool(value != 0, 128);
            if value != 0 {
                writer.write_signed_literal(value, 6);
            }
        }
        writer.write_literal(2, 2); // four coefficient-token partitions
        write_quantization_header(&mut writer, 63, [-7, 0, 4, 0, -3], false);
        write_coefficient_updates(&mut writer, &[], false, 0);
        pad_first_partition(&mut writer);
        let mut partition_zero = writer.finish();
        partition_zero.extend_from_slice(&[0; 8]);

        let mut payload = key_frame(3, 5, 0, true, partition_zero.len() as u32).to_vec();
        payload.extend_from_slice(&partition_zero);
        payload.extend_from_slice(&[1, 0, 0, 2, 0, 0, 0, 0, 0]);
        payload.extend_from_slice(&[0xaa, 0xbb, 0xcc, 0xdd]);
        let frame = parse_riff_payload(&payload, None, &DecodeLimits::default()).unwrap();
        let layout = parse_partition_layout(&payload, &frame, &DecodeLimits::default()).unwrap();

        assert!(!layout.header.colorspace_reserved);
        assert!(layout.header.clamp_type);
        assert_eq!(layout.header.token_partition_count, 4);
        assert_eq!(layout.header.segments.quantizer, [-5, 0, 3, 0]);
        assert_eq!(layout.header.segments.filter_strength, [-4, 0, 0, 0]);
        assert_eq!(layout.header.segments.probabilities, [11, 255, 77]);
        assert_eq!(layout.header.filter.level, 17);
        assert_eq!(layout.header.filter.sharpness, 4);
        assert_eq!(layout.header.filter.ref_deltas, [2, 0, 0, 0]);
        assert_eq!(layout.header.filter.mode_deltas, [0, 0, 0, -1]);
        assert_eq!(
            layout.header.quantization,
            QuantizationHeader {
                base_index: 63,
                y1_dc_delta: -7,
                y2_dc_delta: 0,
                y2_ac_delta: 4,
                uv_dc_delta: 0,
                uv_ac_delta: -3,
            }
        );
        assert!(!layout.header.refresh_entropy_probabilities);
        assert_eq!(layout.header.coefficients.get(0, 0, 0, 0), 128);
        assert_eq!(layout.header.coefficients.get(0, 1, 0, 0), 253);
        assert_eq!(layout.header.coefficients.get(3, 7, 2, 10), 128);
        assert!(!layout.header.coefficients.use_skip_probability);
        assert_eq!(layout.header.coefficients.skip_probability, 0);
        assert_eq!(
            layout
                .tokens
                .iter()
                .map(|part| part.data)
                .collect::<Vec<_>>(),
            vec![&[0xaa][..], &[0xbb, 0xcc], &[], &[0xdd]],
        );
    }

    #[test]
    fn rejects_truncated_or_oversized_token_partition_tables() {
        let mut writer = TestBoolWriter::new();
        writer.write_bool(false, 128); // colour space
        writer.write_bool(false, 128); // clamp type
        writer.write_bool(false, 128); // no segmentation
        writer.write_bool(false, 128); // normal filter
        writer.write_literal(0, 6);
        writer.write_literal(0, 3);
        writer.write_bool(false, 128); // no filter deltas
        writer.write_literal(2, 2); // four token partitions
        write_quantization_header(&mut writer, 0, [0; 5], false);
        write_coefficient_updates(&mut writer, &[], false, 0);
        pad_first_partition(&mut writer);
        let partition_zero = writer.finish();
        let mut payload = key_frame(1, 1, 0, true, partition_zero.len() as u32).to_vec();
        payload.extend_from_slice(&partition_zero);
        let frame = parse_riff_payload(&payload, None, &DecodeLimits::default()).unwrap();
        assert_eq!(
            parse_partition_layout(&payload, &frame, &DecodeLimits::default())
                .unwrap_err()
                .kind(),
            DecodeErrorKind::UnexpectedEof
        );

        payload.extend_from_slice(&[5, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(
            parse_partition_layout(&payload, &frame, &DecodeLimits::default())
                .unwrap_err()
                .kind(),
            DecodeErrorKind::UnexpectedEof
        );
    }

    #[test]
    fn parses_each_legal_token_partition_count() {
        for partition_bits in 0..4_u32 {
            let mut writer = TestBoolWriter::new();
            writer.write_bool(false, 128); // colour space
            writer.write_bool(false, 128); // clamp type
            writer.write_bool(false, 128); // no segmentation
            writer.write_bool(false, 128); // normal filter
            writer.write_literal(0, 6);
            writer.write_literal(0, 3);
            writer.write_bool(false, 128); // no filter deltas
            writer.write_literal(partition_bits, 2);
            write_quantization_header(&mut writer, 0, [0; 5], false);
            write_coefficient_updates(&mut writer, &[], false, 0);
            pad_first_partition(&mut writer);
            let partition_zero = writer.finish();
            let partition_count = 1_usize << partition_bits;
            let mut payload = key_frame(1, 1, 0, true, partition_zero.len() as u32).to_vec();
            payload.extend_from_slice(&partition_zero);
            payload.resize(payload.len() + 3 * (partition_count - 1), 0);
            payload.push(0);

            let frame = parse_riff_payload(&payload, None, &DecodeLimits::default()).unwrap();
            let layout =
                parse_partition_layout(&payload, &frame, &DecodeLimits::default()).unwrap();
            assert_eq!(
                layout.header.token_partition_count as usize,
                partition_count
            );
            assert_eq!(layout.tokens.len(), partition_count);
            assert_eq!(layout.tokens.last().unwrap().data, &[0]);
        }
    }

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
    fn derives_loop_filter_strengths_with_deltas_sharpness_and_segments() {
        let filter = FilterHeader {
            simple: false,
            level: 17,
            sharpness: 4,
            use_deltas: true,
            ref_deltas: [2, 0, 0, 0],
            mode_deltas: [-1, 0, 0, 0],
        };
        let disabled_segments = SegmentHeader {
            enabled: false,
            update_map: false,
            absolute_delta: true,
            quantizer: [0; 4],
            filter_strength: [0; 4],
            probabilities: [255; 3],
        };
        let strengths = derive_loop_filter_strengths(&filter, &disabled_segments);
        assert_eq!(
            strengths[0][0],
            LoopFilterStrength {
                level: 19,
                inner_limit: 5,
                edge_limit: 43,
                hev_threshold: 1,
            }
        );
        assert_eq!(
            strengths[0][1],
            LoopFilterStrength {
                level: 18,
                inner_limit: 5,
                edge_limit: 41,
                hev_threshold: 1,
            }
        );
        assert!(strengths[0][1].filters_inner(true, true));
        assert!(strengths[0][0].filters_inner(false, false));
        assert!(!strengths[0][0].filters_inner(false, true));

        let segments = SegmentHeader {
            enabled: true,
            update_map: true,
            absolute_delta: false,
            quantizer: [0; 4],
            filter_strength: [-30, 50, 0, 80],
            probabilities: [0; 3],
        };
        let segmented = derive_loop_filter_strengths(&filter, &segments);
        assert_eq!(segmented[0], [LoopFilterStrength::default(); 2]);
        assert_eq!(segmented[1][0].level, 63);
        assert_eq!(segmented[1][0].inner_limit, 5);
        assert_eq!(segmented[1][0].edge_limit, 131);
        assert_eq!(segmented[1][0].hev_threshold, 2);
        assert_eq!(segmented[3][1].level, 63);
    }

    #[test]
    fn scalar_loop_filters_match_vp8_two_four_and_six_tap_rules() {
        let mut simple = [100, 100, 100, 110, 110, 110];
        assert!(filter_simple_edge(&mut simple, 3, 1, 25));
        assert_eq!(simple, [100, 100, 102, 107, 110, 110]);

        let hev_strength = LoopFilterStrength {
            level: 20,
            inner_limit: 100,
            edge_limit: 50,
            hev_threshold: 0,
        };
        let mut high_variance = [100, 100, 100, 100, 110, 140, 140, 140];
        assert!(filter_normal_edge(
            &mut high_variance,
            4,
            1,
            hev_strength,
            true
        ));
        assert_eq!(high_variance, [100, 100, 100, 99, 111, 140, 140, 140]);

        let smooth_strength = LoopFilterStrength {
            level: 20,
            inner_limit: 10,
            edge_limit: 100,
            hev_threshold: 20,
        };
        let mut macroblock = [100, 100, 100, 100, 110, 110, 110, 110];
        assert!(filter_normal_edge(
            &mut macroblock,
            4,
            1,
            smooth_strength,
            true
        ));
        assert_eq!(macroblock, [100, 101, 103, 104, 106, 107, 109, 110]);

        let mut inner = [100, 100, 100, 100, 110, 110, 110, 110];
        assert!(filter_normal_edge(&mut inner, 4, 1, smooth_strength, false));
        assert_eq!(inner, [100, 100, 102, 104, 106, 108, 110, 110]);
    }

    #[test]
    fn row_filter_applies_luma_internal_edges_only_when_requested() {
        let strength = LoopFilterStrength {
            level: 10,
            inner_limit: 10,
            edge_limit: 25,
            hev_threshold: 0,
        };
        let mut y = vec![0; 16 * 16];
        for row in y.chunks_exact_mut(16) {
            row[..4].fill(100);
            row[4..].fill(110);
        }
        let mut u = vec![128; 8 * 8];
        let mut v = vec![128; 8 * 8];
        filter_macroblock(MacroblockFilter {
            y: &mut y,
            u: &mut u,
            v: &mut v,
            y_stride: 16,
            uv_stride: 8,
            macroblock_x: 0,
            macroblock_y: 0,
            simple: true,
            strength,
            filters_inner: true,
        });
        for row in y.chunks_exact(16) {
            assert_eq!(&row[2..6], &[100, 102, 107, 110]);
        }

        let mut untouched = vec![0; 16 * 16];
        for row in untouched.chunks_exact_mut(16) {
            row[..4].fill(100);
            row[4..].fill(110);
        }
        filter_macroblock(MacroblockFilter {
            y: &mut untouched,
            u: &mut u,
            v: &mut v,
            y_stride: 16,
            uv_stride: 8,
            macroblock_x: 0,
            macroblock_y: 0,
            simple: true,
            strength,
            filters_inner: false,
        });
        assert!(
            untouched
                .iter()
                .all(|&sample| sample == 100 || sample == 110)
        );
    }

    #[test]
    fn scalar_loop_filters_skip_out_of_bounds_and_sharp_edges() {
        let strength = LoopFilterStrength {
            level: 10,
            inner_limit: 5,
            edge_limit: 10,
            hev_threshold: 0,
        };
        let mut short = [100_u8; 4];
        assert!(!filter_simple_edge(&mut short, 1, 1, 10));
        assert!(!filter_normal_edge(&mut short, 2, 1, strength, true));

        let mut sharp = [0, 0, 0, 0, 255, 255, 255, 255];
        assert!(!filter_normal_edge(&mut sharp, 4, 1, strength, true));
        assert_eq!(sharp, [0, 0, 0, 0, 255, 255, 255, 255]);
    }

    #[test]
    fn inverse_dct_preserves_zero_and_dc_microvectors() {
        assert_eq!(inverse_dct_4x4([0; 16]), [0; 16]);
        let mut dc = [0_i16; 16];
        dc[0] = 16;
        assert_eq!(inverse_dct_4x4(dc), [2; 16]);
    }

    #[test]
    fn inverse_wht_distributes_y2_dc_to_all_macroblock_blocks() {
        assert_eq!(inverse_wht_4x4([0; 16]), [0; 16]);
        let mut dc = [0_i16; 16];
        dc[0] = 8;
        assert_eq!(inverse_wht_4x4(dc), [1; 16]);
    }

    #[test]
    fn widened_transforms_and_macroblock_dequantization_preserve_y2_dc_layout() {
        let mut dc = [0_i32; 16];
        dc[0] = 16;
        assert_eq!(inverse_dct_4x4_i32(dc), [2; 16]);

        let empty = DecodedCoefficients {
            values: [0; 16],
            end: 0,
            non_zero: 0,
        };
        let mut residuals = MacroblockResiduals {
            y2: Some(DecodedCoefficients {
                values: [8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
                end: 1,
                non_zero: 1,
            }),
            luma: [empty; 16],
            u: [empty; 4],
            v: [empty; 4],
            non_zero_y: 0,
            non_zero_uv: 0,
        };
        residuals.luma[0].values[1] = 2;
        residuals.u[0].values[0] = 3;
        residuals.u[0].values[1] = -2;
        let matrix = DequantizationMatrix {
            y1_dc: 2,
            y1_ac: 3,
            y2_dc: 4,
            y2_ac: 5,
            uv_dc: 6,
            uv_ac: 7,
            uv_quant: 0,
        };

        let dequantized = dequantize_macroblock(&residuals, matrix);
        assert_eq!(dequantized.luma[0][0], 4);
        assert_eq!(dequantized.luma[15][0], 4);
        assert_eq!(dequantized.luma[0][1], 6);
        assert_eq!(dequantized.u[0][0], 18);
        assert_eq!(dequantized.u[0][1], -14);
        let spatial = inverse_transform_macroblock(dequantized);
        assert_eq!(spatial.luma[0], inverse_dct_4x4_i32(dequantized.luma[0]));
    }

    #[test]
    fn macroblock_sample_composition_maps_blocks_and_clips_edges() {
        let mut residues = MacroblockSpatialResidues {
            luma: [[0; 16]; 16],
            u: [[0; 16]; 4],
            v: [[0; 16]; 4],
        };
        residues.luma[0][0] = 2;
        residues.luma[5][6] = -3;
        residues.u[3][15] = 200;
        residues.v[0][0] = -200;
        let pixels = combine_macroblock_prediction(
            MacroblockPixels {
                y: [128; 256],
                u: [128; 64],
                v: [128; 64],
            },
            residues,
        );
        assert_eq!(pixels.y[0], 130);
        assert_eq!(pixels.y[5 * 16 + 6], 125);
        assert_eq!(pixels.u[7 * 8 + 7], 255);
        assert_eq!(pixels.v[0], 0);
        assert_eq!(add_residue_and_clip(0, -1), 0);
        assert_eq!(add_residue_and_clip(255, 1), 255);
    }

    #[test]
    fn intra16_prediction_uses_neighbours_and_dc_boundary_fallbacks() {
        let edges = MacroblockPredictionEdges {
            top_y: Some([10; 16]),
            top_right_y: Some([10; 4]),
            left_y: Some([30; 16]),
            top_left_y: 5,
            top_u: Some([50; 8]),
            left_u: Some([70; 8]),
            top_left_u: 20,
            top_v: Some([80; 8]),
            left_v: Some([90; 8]),
            top_left_v: 30,
        };
        let prediction =
            predict_intra16_macroblock(Intra16Mode::Vertical, ChromaMode::Horizontal, edges);
        assert_eq!(prediction.y, [10; 256]);
        assert_eq!(prediction.u, [70; 64]);
        assert_eq!(prediction.v, [90; 64]);

        let true_motion =
            predict_intra16_macroblock(Intra16Mode::TrueMotion, ChromaMode::TrueMotion, edges);
        assert_eq!(true_motion.y, [35; 256]);
        assert_eq!(true_motion.u, [100; 64]);
        assert_eq!(true_motion.v, [140; 64]);

        let dc = predict_intra16_macroblock(
            Intra16Mode::Dc,
            ChromaMode::Dc,
            MacroblockPredictionEdges::default(),
        );
        assert_eq!(dc.y, [128; 256]);
        assert_eq!(dc.u, [128; 64]);
        assert_eq!(dc.v, [128; 64]);
        let sentinel = predict_intra16_macroblock(
            Intra16Mode::Vertical,
            ChromaMode::Horizontal,
            MacroblockPredictionEdges::default(),
        );
        assert_eq!(sentinel.y, [127; 256]);
        assert_eq!(sentinel.u, [129; 64]);
    }

    #[test]
    fn intra4_prediction_covers_all_vp8_directional_modes() {
        let top = [10, 20, 30, 40, 50, 60, 70, 80];
        let left = [50, 60, 70, 80];
        let dc = predict_intra4_block(Intra4Mode::Dc, 5, top, left);
        assert_eq!(dc, [45; 16]);
        let true_motion = predict_intra4_block(Intra4Mode::TrueMotion, 5, top, left);
        assert_eq!(true_motion[0], 55);
        assert_eq!(true_motion[15], 115);
        assert_eq!(
            predict_intra4_block(Intra4Mode::Vertical, 5, top, left),
            [
                11, 20, 30, 40, 11, 20, 30, 40, 11, 20, 30, 40, 11, 20, 30, 40
            ]
        );
        assert_eq!(
            predict_intra4_block(Intra4Mode::Horizontal, 5, top, left),
            [
                41, 41, 41, 41, 60, 60, 60, 60, 70, 70, 70, 70, 78, 78, 78, 78
            ]
        );
        for mode in [
            Intra4Mode::DiagonalDownRight,
            Intra4Mode::VerticalRight,
            Intra4Mode::DiagonalDownLeft,
            Intra4Mode::VerticalLeft,
            Intra4Mode::HorizontalDown,
            Intra4Mode::HorizontalUp,
        ] {
            let prediction = predict_intra4_block(mode, 5, top, left);
            assert_ne!(prediction, [128; 16], "{mode:?}");
        }
        let diagonal_left = predict_intra4_block(Intra4Mode::DiagonalDownLeft, 5, top, left);
        assert_eq!(diagonal_left[0], 20);
        assert_eq!(diagonal_left[15], 78);
        let horizontal_up = predict_intra4_block(Intra4Mode::HorizontalUp, 5, top, left);
        assert_eq!(horizontal_up[12..], [80; 4]);
    }

    #[test]
    fn intra4_macroblock_and_full_reconstruction_follow_raster_neighbours() {
        let edges = MacroblockPredictionEdges {
            top_y: Some([10; 16]),
            top_right_y: Some([10; 4]),
            left_y: Some([30; 16]),
            top_left_y: 5,
            ..MacroblockPredictionEdges::default()
        };
        let prediction = predict_intra4_macroblock([Intra4Mode::Dc; 16], edges);
        assert_eq!(prediction[0], 20);
        assert_eq!(prediction[4], 15);

        let empty = DecodedCoefficients {
            values: [0; 16],
            end: 0,
            non_zero: 0,
        };
        let residuals = MacroblockResiduals {
            y2: None,
            luma: [empty; 16],
            u: [empty; 4],
            v: [empty; 4],
            non_zero_y: 0,
            non_zero_uv: 0,
        };
        let pixels = reconstruct_intra_macroblock(
            IntraMacroblock {
                segment: 0,
                skip: true,
                luma: LumaMode::FourByFour([Intra4Mode::Dc; 16]),
                chroma: ChromaMode::Dc,
            },
            &residuals,
            DequantizationMatrix {
                y1_dc: 1,
                y1_ac: 1,
                y2_dc: 1,
                y2_ac: 1,
                uv_dc: 1,
                uv_ac: 1,
                uv_quant: 0,
            },
            MacroblockPredictionEdges::default(),
        )
        .unwrap();
        assert!(pixels.y[..64].iter().all(|&value| value == 128));
        assert!(pixels.y[64..].iter().all(|&value| value == 129));
        assert_eq!(pixels.u, [128; 64]);
        assert_eq!(pixels.v, [128; 64]);
    }

    #[test]
    fn coefficient_decoder_handles_eob_zero_runs_signs_and_zigzag() {
        let probabilities = CoefficientProbabilities::default();
        let mut writer = TestBoolWriter::new();

        let initial = coefficient_nodes(&probabilities, CoefficientBlockType::Luma16Ac, 0, 0);
        writer.write_bool(true, initial[0]); // not EOB
        writer.write_bool(false, initial[1]); // zero at position zero
        let position_one = coefficient_nodes(&probabilities, CoefficientBlockType::Luma16Ac, 1, 0);
        writer.write_bool(false, position_one[1]); // zero at position one
        let position_two = coefficient_nodes(&probabilities, CoefficientBlockType::Luma16Ac, 2, 0);
        writer.write_bool(true, position_two[1]);
        writer.write_bool(false, position_two[2]); // magnitude one
        writer.write_bool(true, 128); // negative
        let next = coefficient_nodes(&probabilities, CoefficientBlockType::Luma16Ac, 3, 1);
        writer.write_bool(false, next[0]); // EOB

        let bytes = writer.finish();
        let mut decoder = BoolDecoder::new(&bytes, &DecodeLimits::default()).unwrap();
        let decoded = decode_coefficients(
            &mut decoder,
            &probabilities,
            CoefficientBlockType::Luma16Ac,
            0,
            0,
        )
        .unwrap();
        let mut expected = [0_i16; 16];
        expected[COEFFICIENT_ZIGZAG[2]] = -1;
        assert_eq!(decoded.values, expected);
        assert_eq!(decoded.end, 3);
        assert_eq!(decoded.non_zero, 1);
    }

    #[test]
    fn coefficient_decoder_handles_large_category_values_and_ac_only_start() {
        let probabilities = CoefficientProbabilities::default();
        let mut writer = TestBoolWriter::new();

        let nodes = coefficient_nodes(&probabilities, CoefficientBlockType::Luma4Ac, 1, 2);
        writer.write_bool(true, nodes[0]); // not EOB
        writer.write_bool(true, nodes[1]); // non-zero
        writer.write_bool(true, nodes[2]); // value exceeds one
        writer.write_bool(true, nodes[3]); // category path
        writer.write_bool(true, nodes[6]);
        writer.write_bool(false, nodes[8]);
        writer.write_bool(true, nodes[9]);
        for &probability in CATEGORY_PROBABILITIES[2] {
            writer.write_bool(false, probability);
        }
        writer.write_bool(false, 128); // positive sign
        let next = coefficient_nodes(&probabilities, CoefficientBlockType::Luma4Ac, 2, 2);
        writer.write_bool(false, next[0]); // EOB

        let bytes = writer.finish();
        let mut decoder = BoolDecoder::new(&bytes, &DecodeLimits::default()).unwrap();
        let decoded = decode_coefficients(
            &mut decoder,
            &probabilities,
            CoefficientBlockType::Luma4Ac,
            2,
            1,
        )
        .unwrap();
        let mut expected = [0_i16; 16];
        // This uses the category-five branch (base magnitude 35) with a zero
        // suffix, exercising the longest category tree selected by this
        // compact vector.
        expected[COEFFICIENT_ZIGZAG[1]] = 35;
        assert_eq!(decoded.values, expected);
        assert_eq!(decoded.end, 2);
        assert_eq!(decoded.non_zero, 1);
    }

    #[test]
    fn coefficient_decoder_rejects_invalid_context_and_start() {
        let probabilities = CoefficientProbabilities::default();
        let mut decoder = BoolDecoder::new(&[0], &DecodeLimits::default()).unwrap();
        assert_eq!(
            decode_coefficients(
                &mut decoder,
                &probabilities,
                CoefficientBlockType::LumaDc,
                3,
                0,
            )
            .unwrap_err()
            .kind(),
            DecodeErrorKind::InvalidParameter
        );
        assert_eq!(
            decode_coefficients(
                &mut decoder,
                &probabilities,
                CoefficientBlockType::LumaDc,
                0,
                16,
            )
            .unwrap_err()
            .kind(),
            DecodeErrorKind::InvalidParameter
        );
    }

    #[test]
    fn intra_mode_row_parses_segments_skip_and_sixteen_by_sixteen_modes() {
        let coefficients = CoefficientProbabilities {
            use_skip_probability: true,
            skip_probability: 128,
            ..CoefficientProbabilities::default()
        };
        let header = first_partition_header(
            SegmentHeader {
                enabled: true,
                update_map: true,
                absolute_delta: true,
                quantizer: [0; 4],
                filter_strength: [0; 4],
                probabilities: [128; 3],
            },
            coefficients,
        );
        let mut writer = TestBoolWriter::new();

        writer.write_bool(false, 128); // segment branch 0/1
        writer.write_bool(true, 128); // segment one
        writer.write_bool(true, 128); // skip
        writer.write_bool(true, 145); // 16x16 luma
        writer.write_bool(false, 156);
        writer.write_bool(true, 163); // vertical luma
        writer.write_bool(true, 142);
        writer.write_bool(false, 114); // vertical chroma

        writer.write_bool(true, 128); // segment branch 2/3
        writer.write_bool(false, 128); // segment two
        writer.write_bool(false, 128); // not skipped
        writer.write_bool(true, 145); // 16x16 luma
        writer.write_bool(true, 156);
        writer.write_bool(false, 128); // horizontal luma
        writer.write_bool(true, 142);
        writer.write_bool(true, 114);
        writer.write_bool(true, 183); // true-motion chroma

        let bytes = writer.finish();
        let mut decoder = BoolDecoder::new(&bytes, &DecodeLimits::default()).unwrap();
        let mut top = [Intra4Mode::Dc; 8];
        let mut blocks = [IntraMacroblock {
            segment: 0,
            skip: false,
            luma: LumaMode::Sixteen(Intra16Mode::Dc),
            chroma: ChromaMode::Dc,
        }; 2];
        parse_intra_mode_row(&mut decoder, &header, &mut top, &mut blocks).unwrap();

        assert_eq!(blocks[0].segment, 1);
        assert!(blocks[0].skip);
        assert_eq!(blocks[0].luma, LumaMode::Sixteen(Intra16Mode::Vertical));
        assert_eq!(blocks[0].chroma, ChromaMode::Vertical);
        assert_eq!(blocks[1].segment, 2);
        assert!(!blocks[1].skip);
        assert_eq!(blocks[1].luma, LumaMode::Sixteen(Intra16Mode::Horizontal));
        assert_eq!(blocks[1].chroma, ChromaMode::TrueMotion);
        assert_eq!(
            top,
            [
                Intra4Mode::Vertical,
                Intra4Mode::Vertical,
                Intra4Mode::Vertical,
                Intra4Mode::Vertical,
                Intra4Mode::Horizontal,
                Intra4Mode::Horizontal,
                Intra4Mode::Horizontal,
                Intra4Mode::Horizontal,
            ]
        );
    }

    #[test]
    fn intra_mode_row_decodes_four_by_four_modes_and_validates_context_shape() {
        let header = first_partition_header(
            SegmentHeader {
                enabled: false,
                update_map: false,
                absolute_delta: true,
                quantizer: [0; 4],
                filter_strength: [0; 4],
                probabilities: [255; 3],
            },
            CoefficientProbabilities::default(),
        );
        let mut writer = TestBoolWriter::new();
        writer.write_bool(false, 145); // 4x4 luma
        for _ in 0..16 {
            writer.write_bool(false, 231); // B_DC_PRED given DC top and left
        }
        writer.write_bool(false, 142); // DC chroma

        let bytes = writer.finish();
        let mut decoder = BoolDecoder::new(&bytes, &DecodeLimits::default()).unwrap();
        let mut top = [Intra4Mode::Dc; 4];
        let mut blocks = [IntraMacroblock {
            segment: 0,
            skip: false,
            luma: LumaMode::Sixteen(Intra16Mode::Dc),
            chroma: ChromaMode::Dc,
        }];
        parse_intra_mode_row(&mut decoder, &header, &mut top, &mut blocks).unwrap();
        assert_eq!(blocks[0].luma, LumaMode::FourByFour([Intra4Mode::Dc; 16]));
        assert_eq!(blocks[0].chroma, ChromaMode::Dc);
        assert_eq!(top, [Intra4Mode::Dc; 4]);

        let mut no_top = [];
        assert_eq!(
            parse_intra_mode_row(&mut decoder, &header, &mut no_top, &mut blocks)
                .unwrap_err()
                .kind(),
            DecodeErrorKind::InvalidParameter
        );
    }

    #[test]
    fn intra_frame_decoder_reconstructs_a_skipped_macroblock() {
        let mut writer = TestBoolWriter::new();
        writer.write_bool(false, 128); // colour space
        writer.write_bool(false, 128); // clamp type
        writer.write_bool(false, 128); // no segmentation
        writer.write_bool(false, 128); // normal filter
        writer.write_literal(0, 6); // filter level
        writer.write_literal(0, 3); // filter sharpness
        writer.write_bool(false, 128); // no filter deltas
        writer.write_literal(0, 2); // one token partition
        write_quantization_header(&mut writer, 0, [0; 5], false);
        write_coefficient_updates(&mut writer, &[], true, 1);
        writer.write_bool(true, 1); // skip residuals
        writer.write_bool(true, 145); // 16x16 luma
        writer.write_bool(false, 156); // DC luma
        writer.write_bool(false, 163);
        writer.write_bool(false, 142); // DC chroma
        let partition_zero = writer.finish();
        let mut payload = key_frame(1, 1, 0, true, partition_zero.len() as u32).to_vec();
        payload.extend_from_slice(&partition_zero);
        payload.push(0); // Non-empty final token partition, never consumed by skip.

        let limits = DecodeLimits::default();
        let frame = parse_riff_payload(&payload, None, &limits).unwrap();
        let image = decode_intra_frame(&payload, &frame, &limits).unwrap();
        assert_eq!(image.width, 1);
        assert_eq!(image.height, 1);
        assert_eq!(image.y_stride, 16);
        assert_eq!(image.uv_stride, 8);
        assert_eq!(image.y.len(), 16 * 16);
        assert_eq!(image.u.len(), 8 * 8);
        assert!(image.y.iter().all(|&sample| sample == 128));
        assert!(image.u.iter().all(|&sample| sample == 128));
        assert!(image.v.iter().all(|&sample| sample == 128));
    }

    #[test]
    fn macroblock_storage_exposes_reconstructed_edges() {
        let frame = Vp8Header {
            width: 17,
            height: 17,
            version: 0,
            first_partition_len: 0,
            horizontal_scale: 0,
            vertical_scale: 0,
        };
        let mut image = Vp8YuvImage::new(&frame, &DecodeLimits::default()).unwrap();
        let pixels = MacroblockPixels {
            y: std::array::from_fn(|index| index as u8),
            u: std::array::from_fn(|index| (index + 64) as u8),
            v: std::array::from_fn(|index| (index + 128) as u8),
        };
        image.store_macroblock(0, 0, pixels);
        let right_edges = image.edges(1, 0);
        assert_eq!(
            right_edges.left_y.unwrap(),
            std::array::from_fn(|row| (row * 16 + 15) as u8)
        );
        assert_eq!(
            right_edges.left_u.unwrap(),
            std::array::from_fn(|row| (row * 8 + 71) as u8)
        );
        assert_eq!(
            right_edges.left_v.unwrap(),
            std::array::from_fn(|row| (row * 8 + 135) as u8)
        );
        let below_edges = image.edges(0, 1);
        assert_eq!(below_edges.top_y.unwrap(), pixels.y[240..256]);
        assert_eq!(below_edges.top_u.unwrap(), pixels.u[56..64]);
        assert_eq!(below_edges.top_v.unwrap(), pixels.v[56..64]);
    }

    #[test]
    fn macroblock_storage_enforces_allocation_limit() {
        let frame = Vp8Header {
            width: 1,
            height: 1,
            version: 0,
            first_partition_len: 0,
            horizontal_scale: 0,
            vertical_scale: 0,
        };
        let limits = DecodeLimits {
            max_alloc_bytes: 383,
            ..DecodeLimits::default()
        };
        assert_eq!(
            Vp8YuvImage::new(&frame, &limits).unwrap_err().kind(),
            DecodeErrorKind::LimitExceeded
        );
    }

    #[test]
    fn yuv_image_converts_visible_rectangle_to_vp8_rgba() {
        let image = Vp8YuvImage {
            width: 2,
            height: 2,
            y_stride: 2,
            uv_stride: 1,
            y: vec![16, 235, 81, 145],
            u: vec![128],
            v: vec![128],
        };
        assert_eq!(
            image.to_rgba(&DecodeLimits::default()).unwrap(),
            vec![
                0, 0, 0, 255, 255, 255, 255, 255, 76, 76, 76, 255, 150, 150, 150, 255
            ]
        );
    }

    #[test]
    fn yuv_image_rejects_short_visible_plane() {
        let image = Vp8YuvImage {
            width: 2,
            height: 2,
            y_stride: 2,
            uv_stride: 1,
            y: vec![0; 3],
            u: vec![128],
            v: vec![128],
        };
        assert_eq!(
            image.to_rgba(&DecodeLimits::default()).unwrap_err().kind(),
            DecodeErrorKind::InvalidParameter
        );
    }

    #[test]
    fn residual_decoder_consumes_all_intra_block_families_and_preserves_empty_contexts() {
        let probabilities = CoefficientProbabilities::default();
        let mut writer = TestBoolWriter::new();
        write_coefficient_eob(
            &mut writer,
            &probabilities,
            CoefficientBlockType::LumaDc,
            0,
            0,
        );
        for _ in 0..16 {
            write_coefficient_eob(
                &mut writer,
                &probabilities,
                CoefficientBlockType::Luma16Ac,
                1,
                0,
            );
        }
        for _ in 0..8 {
            write_coefficient_eob(
                &mut writer,
                &probabilities,
                CoefficientBlockType::ChromaAc,
                0,
                0,
            );
        }

        let bytes = writer.finish();
        let mut decoder = BoolDecoder::new(&bytes, &DecodeLimits::default()).unwrap();
        let mut top = ResidualContext::default();
        let mut left = ResidualContext::default();
        let residuals =
            decode_intra_residuals(&mut decoder, &probabilities, false, &mut top, &mut left)
                .unwrap();
        assert_eq!(residuals.y2.unwrap().end, 0);
        assert!(residuals.luma.iter().all(|block| block.non_zero == 0));
        assert!(residuals.u.iter().all(|block| block.non_zero == 0));
        assert!(residuals.v.iter().all(|block| block.non_zero == 0));
        assert_eq!(residuals.non_zero_y, 0);
        assert_eq!(residuals.non_zero_uv, 0);
        assert_eq!(top, ResidualContext::default());
        assert_eq!(left, ResidualContext::default());
    }
}
