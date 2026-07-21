//! Minimal VP8 key-frame writer foundations.
//!
//! The first M7 slice emits an intra-only DC-predicted key frame with zero
//! residuals. It is intentionally a format-validation primitive, not yet the
//! public RGB(A) lossy encoder: RGB-to-YUV, transform/quantization and
//! coefficient selection build on this verified partition layout.

use crate::coefficients::{
    COEFFICIENT_BANDS, COEFFICIENT_DEFAULTS, COEFFICIENT_UPDATE_PROBABILITIES,
    COEFFICIENT_ZIGZAG,
};
use crate::{
    BoolEncodeError, BoolEncoder, ChromaMode, CoefficientBlockType, CoefficientEncodeError,
    CoefficientProbabilities, DecodedCoefficients, DequantizationMatrix, Intra16Mode,
    IntraMacroblock, LumaMode, MacroblockResiduals, QuantizationHeader, SegmentHeader,
    Vp8YuvImage, derive_dequantization, encode_coefficients, forward_dct_4x4_i32,
    forward_wht_4x4_i32, predict_intra16_macroblock, quantize_block,
    reconstruct_intra_macroblock,
};

const KEY_FRAME_HEADER_LEN: usize = 10;
const KEY_FRAME_START_CODE: [u8; 3] = [0x9d, 0x01, 0x2a];

/// Failure while constructing a VP8 key frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Vp8EncodeError {
    InvalidDimensions,
    InvalidRgbaLength,
    AllocationFailed,
    FirstPartitionTooLarge,
    InvalidPlaneLayout,
    InvalidQuantizer,
}

/// Macroblock-aligned VP8 YUV420 source planes prepared from straight RGBA8.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Vp8SourceYuv {
    pub width: u32,
    pub height: u32,
    pub y_stride: usize,
    pub uv_stride: usize,
    pub y: Vec<u8>,
    pub u: Vec<u8>,
    pub v: Vec<u8>,
}

/// Quantized residual coefficients for one DC-predicted VP8 macroblock.
///
/// The luma block DC values are represented by `y2`; every `luma` block has
/// its DC entry cleared, as VP8 requires for 16×16 intra prediction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Vp8DcMacroblockCoefficients {
    pub y2: [i16; 16],
    pub luma: [[i16; 16]; 16],
    pub u: [[i16; 16]; 4],
    pub v: [[i16; 16]; 4],
}

impl From<BoolEncodeError> for Vp8EncodeError {
    fn from(error: BoolEncodeError) -> Self {
        match error {
            BoolEncodeError::AllocationFailed => Self::AllocationFailed,
            BoolEncodeError::InvalidLiteralWidth => Self::FirstPartitionTooLarge,
        }
    }
}

/// Emits a valid visible VP8 key frame with DC intra prediction and zero
/// residual coefficients.
///
/// This intentionally produces the codec's neutral prediction rather than
/// encoding supplied pixels. It is a public, independently testable M7
/// bitstream-building primitive; the RGBA encoder is added only after it can
/// populate this frame with quantized coefficients.
pub fn encode_neutral_key_frame(width: u32, height: u32) -> Result<Vec<u8>, Vp8EncodeError> {
    if width == 0 || height == 0 || width > 0x3fff || height > 0x3fff {
        return Err(Vp8EncodeError::InvalidDimensions);
    }
    let macroblock_width =
        usize::try_from(width.div_ceil(16)).map_err(|_| Vp8EncodeError::InvalidDimensions)?;
    let macroblock_height =
        usize::try_from(height.div_ceil(16)).map_err(|_| Vp8EncodeError::InvalidDimensions)?;
    let macroblock_count = macroblock_width
        .checked_mul(macroblock_height)
        .ok_or(Vp8EncodeError::AllocationFailed)?;
    let blocks = vec![dc_intra_macroblock(); macroblock_count];
    let first_partition =
        write_first_partition(&blocks, 0, &CoefficientProbabilities::default(), None)?;
    let token_partition = write_zero_token_partition(macroblock_count)?;
    assemble_key_frame(width, height, first_partition, token_partition)
}

/// Emits a 16×16 VP8 key frame from one DC-predicted YUV macroblock.
///
/// This deliberately narrow primitive is the first non-neutral VP8 output
/// slice: it exercises the real transform, quantizer, Y2 path, and
/// coefficient token partition. The forthcoming frame writer generalizes its
/// neighbour contexts and prediction borders to arbitrary dimensions.
pub fn encode_dc_predicted_macroblock_key_frame(
    source: &Vp8SourceYuv,
) -> Result<Vec<u8>, Vp8EncodeError> {
    encode_dc_predicted_macroblock_key_frame_with_quantizer(source, 0)
}

/// Emits a 16×16 VP8 key frame with the supplied VP8 base quantizer.
///
/// `quantizer` is the exact 0 through 127 value written into the first
/// partition. The output remains limited to one DC-predicted macroblock while
/// the general frame writer is developed.
pub fn encode_dc_predicted_macroblock_key_frame_with_quantizer(
    source: &Vp8SourceYuv,
    quantizer: u8,
) -> Result<Vec<u8>, Vp8EncodeError> {
    if source.width != 16 || source.height != 16 {
        return Err(Vp8EncodeError::InvalidDimensions);
    }
    encode_dc_predicted_key_frame_with_quantizer(source, quantizer)
}

/// Emits a visible VP8 key frame from DC-predicted macroblocks.
///
/// Every visible dimension supported by VP8 is accepted. The source planes
/// must be macroblock padded as produced by [`rgba_to_yuv420`]. Each
/// macroblock is reconstructed locally before its neighbours are encoded, so
/// its residuals use the same DC prediction borders that a decoder will see.
pub fn encode_dc_predicted_key_frame_with_quantizer(
    source: &Vp8SourceYuv,
    quantizer: u8,
) -> Result<Vec<u8>, Vp8EncodeError> {
    if source.width == 0 || source.height == 0 || source.width > 0x3fff || source.height > 0x3fff {
        return Err(Vp8EncodeError::InvalidDimensions);
    }
    if quantizer > 127 {
        return Err(Vp8EncodeError::InvalidQuantizer);
    }
    let macroblock_width = usize::try_from(source.width.div_ceil(16))
        .map_err(|_| Vp8EncodeError::InvalidDimensions)?;
    let macroblock_height = usize::try_from(source.height.div_ceil(16))
        .map_err(|_| Vp8EncodeError::InvalidDimensions)?;
    let macroblock_count = macroblock_width
        .checked_mul(macroblock_height)
        .ok_or(Vp8EncodeError::AllocationFailed)?;
    let y_width = macroblock_width
        .checked_mul(16)
        .ok_or(Vp8EncodeError::InvalidPlaneLayout)?;
    let uv_width = macroblock_width
        .checked_mul(8)
        .ok_or(Vp8EncodeError::InvalidPlaneLayout)?;
    let y_height = macroblock_height
        .checked_mul(16)
        .ok_or(Vp8EncodeError::InvalidPlaneLayout)?;
    let uv_height = macroblock_height
        .checked_mul(8)
        .ok_or(Vp8EncodeError::InvalidPlaneLayout)?;
    if source.y_stride < y_width
        || source.uv_stride < uv_width
        || !has_plane_extent(&source.y, source.y_stride, y_width, y_height)
        || !has_plane_extent(&source.u, source.uv_stride, uv_width, uv_height)
        || !has_plane_extent(&source.v, source.uv_stride, uv_width, uv_height)
    {
        return Err(Vp8EncodeError::InvalidPlaneLayout);
    }
    let matrix = quantization_matrix(quantizer);
    let reconstructed_y_len = y_width
        .checked_mul(y_height)
        .ok_or(Vp8EncodeError::AllocationFailed)?;
    let reconstructed_uv_len = uv_width
        .checked_mul(uv_height)
        .ok_or(Vp8EncodeError::AllocationFailed)?;
    let mut reconstructed = Vp8YuvImage {
        width: source.width,
        height: source.height,
        y_stride: y_width,
        uv_stride: uv_width,
        y: reserve_zeroed(reconstructed_y_len)?,
        u: reserve_zeroed(reconstructed_uv_len)?,
        v: reserve_zeroed(reconstructed_uv_len)?,
    };
    let mut blocks = Vec::new();
    blocks
        .try_reserve_exact(macroblock_count)
        .map_err(|_| Vp8EncodeError::AllocationFailed)?;
    let mut macroblocks = Vec::new();
    macroblocks
        .try_reserve_exact(macroblock_count)
        .map_err(|_| Vp8EncodeError::AllocationFailed)?;
    for macroblock_y in 0..macroblock_height {
        for macroblock_x in 0..macroblock_width {
            let y_offset = macroblock_y * 16 * source.y_stride + macroblock_x * 16;
            let uv_offset = macroblock_y * 8 * source.uv_stride + macroblock_x * 8;
            let edges = reconstructed.edges(macroblock_x, macroblock_y);
            let (mut block, coefficients, pixels) = select_intra16_macroblock(
                &source.y[y_offset..],
                source.y_stride,
                &source.u[uv_offset..],
                &source.v[uv_offset..],
                source.uv_stride,
                matrix,
                edges,
            )?;
            block.skip = macroblock_is_zero(coefficients);
            reconstructed.store_macroblock(macroblock_x, macroblock_y, pixels);
            blocks.push(block);
            macroblocks.push(coefficients);
        }
    }
    let (regular_first, regular_tokens) = write_best_coefficient_partitions(
        &macroblocks,
        &blocks,
        macroblock_width,
        quantizer,
        false,
        None,
    )?;
    let (first_partition, token_partition) = if let Some(skip_probability) = skip_probability(&blocks)
    {
        let (skip_first, skip_tokens) = write_best_coefficient_partitions(
            &macroblocks,
            &blocks,
            macroblock_width,
            quantizer,
            true,
            Some(skip_probability),
        )?;
        let skip_size = partition_pair_len(&skip_first, &skip_tokens)?;
        let regular_size = partition_pair_len(&regular_first, &regular_tokens)?;
        if skip_size < regular_size {
            (skip_first, skip_tokens)
        } else {
            (regular_first, regular_tokens)
        }
    } else {
        (regular_first, regular_tokens)
    };
    assemble_key_frame(source.width, source.height, first_partition, token_partition)
}

fn dc_intra_macroblock() -> IntraMacroblock {
    IntraMacroblock {
        segment: 0,
        skip: false,
        luma: LumaMode::Sixteen(Intra16Mode::Dc),
        chroma: ChromaMode::Dc,
    }
}

fn select_intra16_macroblock(
    y: &[u8],
    y_stride: usize,
    u: &[u8],
    v: &[u8],
    uv_stride: usize,
    matrix: DequantizationMatrix,
    edges: crate::MacroblockPredictionEdges,
) -> Result<(IntraMacroblock, Vp8DcMacroblockCoefficients, crate::MacroblockPixels), Vp8EncodeError> {
    if y_stride < 16
        || uv_stride < 8
        || !has_plane_extent(y, y_stride, 16, 16)
        || !has_plane_extent(u, uv_stride, 8, 8)
        || !has_plane_extent(v, uv_stride, 8, 8)
    {
        return Err(Vp8EncodeError::InvalidPlaneLayout);
    }
    let mut best_luma = None;
    for luma_mode in [
        Intra16Mode::Dc,
        Intra16Mode::Vertical,
        Intra16Mode::Horizontal,
        Intra16Mode::TrueMotion,
    ] {
        let prediction = predict_intra16_macroblock(luma_mode, ChromaMode::Dc, edges);
        let (y2, luma) = quantize_luma_plane(y, y_stride, &prediction.y, matrix);
        let mut coefficients = empty_dc_coefficients();
        coefficients.y2 = y2;
        coefficients.luma = luma;
        let pixels = crate::reconstruction::reconstruct_intra16_luma(
            luma_mode,
            &dc_macroblock_residuals(coefficients),
            matrix,
            edges,
        );
        let score = (
            luma_distortion(y, y_stride, &pixels),
            luma_coefficient_cost(y2, luma),
        );
        if best_luma.is_none_or(|(best_score, _, _, _)| score < best_score) {
            best_luma = Some((score, luma_mode, y2, luma));
        }
    }
    let mut best_chroma = None;
    for chroma_mode in [
        ChromaMode::Dc,
        ChromaMode::Vertical,
        ChromaMode::Horizontal,
        ChromaMode::TrueMotion,
    ] {
        let prediction = predict_intra16_macroblock(Intra16Mode::Dc, chroma_mode, edges);
        let u_coefficients = quantize_chroma_plane(u, uv_stride, &prediction.u, matrix);
        let v_coefficients = quantize_chroma_plane(v, uv_stride, &prediction.v, matrix);
        let mut coefficients = empty_dc_coefficients();
        coefficients.u = u_coefficients;
        coefficients.v = v_coefficients;
        let (pixels_u, pixels_v) = crate::reconstruction::reconstruct_intra16_chroma(
            chroma_mode,
            &dc_macroblock_residuals(coefficients),
            matrix,
            edges,
        );
        let score = (
            chroma_distortion(u, v, uv_stride, &pixels_u, &pixels_v),
            chroma_coefficient_cost(u_coefficients, v_coefficients),
        );
        if best_chroma.is_none_or(|(best_score, _, _, _)| score < best_score) {
            best_chroma = Some((score, chroma_mode, u_coefficients, v_coefficients));
        }
    }
    let (_, luma_mode, y2, luma) = best_luma.ok_or(Vp8EncodeError::InvalidPlaneLayout)?;
    let (_, chroma_mode, u, v) = best_chroma.ok_or(Vp8EncodeError::InvalidPlaneLayout)?;
    let block = IntraMacroblock {
        segment: 0,
        skip: false,
        luma: LumaMode::Sixteen(luma_mode),
        chroma: chroma_mode,
    };
    let coefficients = Vp8DcMacroblockCoefficients { y2, luma, u, v };
    let pixels = reconstruct_intra_macroblock(
        block,
        &dc_macroblock_residuals(coefficients),
        matrix,
        edges,
    )
    .map_err(|_| Vp8EncodeError::InvalidPlaneLayout)?;
    Ok((block, coefficients, pixels))
}

fn empty_dc_coefficients() -> Vp8DcMacroblockCoefficients {
    Vp8DcMacroblockCoefficients {
        y2: [0; 16],
        luma: [[0; 16]; 16],
        u: [[0; 16]; 4],
        v: [[0; 16]; 4],
    }
}

fn macroblock_is_zero(coefficients: Vp8DcMacroblockCoefficients) -> bool {
    coefficients
        .y2
        .into_iter()
        .chain(coefficients.luma.into_iter().flatten())
        .chain(coefficients.u.into_iter().flatten())
        .chain(coefficients.v.into_iter().flatten())
        .all(|value| value == 0)
}

fn skip_probability(blocks: &[IntraMacroblock]) -> Option<u8> {
    let skipped = blocks.iter().filter(|block| block.skip).count();
    if skipped == 0 {
        return None;
    }
    let non_skipped = blocks.len() - skipped;
    Some(((non_skipped * 255) / blocks.len()) as u8)
}

fn partition_pair_len(first: &[u8], tokens: &[u8]) -> Result<usize, Vp8EncodeError> {
    first
        .len()
        .checked_add(tokens.len())
        .ok_or(Vp8EncodeError::AllocationFailed)
}

fn write_best_coefficient_partitions(
    macroblocks: &[Vp8DcMacroblockCoefficients],
    blocks: &[IntraMacroblock],
    macroblock_width: usize,
    quantizer: u8,
    honor_skip: bool,
    skip_probability: Option<u8>,
) -> Result<(Vec<u8>, Vec<u8>), Vp8EncodeError> {
    let defaults = CoefficientProbabilities::default();
    let default_first = write_first_partition(blocks, quantizer, &defaults, skip_probability)?;
    let default_tokens = write_dc_macroblocks_token_partition(
        macroblocks,
        blocks,
        macroblock_width,
        honor_skip,
        &defaults,
    )?;
    let statistics =
        collect_coefficient_statistics(macroblocks, blocks, macroblock_width, honor_skip)?;
    let adapted = statistics.adapted_probabilities();
    if adapted == defaults {
        return Ok((default_first, default_tokens));
    }
    let adapted_first = write_first_partition(blocks, quantizer, &adapted, skip_probability)?;
    let adapted_tokens = write_dc_macroblocks_token_partition(
        macroblocks,
        blocks,
        macroblock_width,
        honor_skip,
        &adapted,
    )?;
    if partition_pair_len(&adapted_first, &adapted_tokens)?
        < partition_pair_len(&default_first, &default_tokens)?
    {
        Ok((adapted_first, adapted_tokens))
    } else {
        Ok((default_first, default_tokens))
    }
}

type CoefficientNodeCounts = [[[[[u64; 2]; 11]; 3]; 8]; 4];

struct CoefficientStatistics {
    nodes: CoefficientNodeCounts,
}

impl CoefficientStatistics {
    fn new() -> Self {
        Self {
            nodes: [[[[[0; 2]; 11]; 3]; 8]; 4],
        }
    }

    fn record(
        &mut self,
        coefficient_type: CoefficientBlockType,
        context: u8,
        start: u8,
        values: [i16; 16],
    ) -> Result<(), Vp8EncodeError> {
        if context > 2 || start >= 16 || values[..usize::from(start)].iter().any(|&value| value != 0)
        {
            return Err(Vp8EncodeError::InvalidPlaneLayout);
        }
        let mut position = usize::from(start);
        let mut coefficient_context = usize::from(context);
        while position < 16 {
            let has_more = ((position)..16).any(|next| values[COEFFICIENT_ZIGZAG[next]] != 0);
            self.record_node(coefficient_type, position, coefficient_context, 0, has_more);
            if !has_more {
                return Ok(());
            }
            while values[COEFFICIENT_ZIGZAG[position]] == 0 {
                self.record_node(coefficient_type, position, coefficient_context, 1, false);
                position += 1;
                coefficient_context = 0;
            }
            self.record_node(coefficient_type, position, coefficient_context, 1, true);
            let value = values[COEFFICIENT_ZIGZAG[position]];
            let magnitude = value.unsigned_abs();
            self.record_node(coefficient_type, position, coefficient_context, 2, magnitude != 1);
            if magnitude > 1 {
                self.record_magnitude(coefficient_type, position, coefficient_context, magnitude)?;
            }
            position += 1;
            coefficient_context = if magnitude == 1 { 1 } else { 2 };
        }
        Ok(())
    }

    fn record_magnitude(
        &mut self,
        coefficient_type: CoefficientBlockType,
        position: usize,
        context: usize,
        magnitude: u16,
    ) -> Result<(), Vp8EncodeError> {
        let at_least_five = magnitude >= 5;
        self.record_node(coefficient_type, position, context, 3, at_least_five);
        if !at_least_five {
            let at_least_three = magnitude >= 3;
            self.record_node(coefficient_type, position, context, 4, at_least_three);
            if at_least_three {
                self.record_node(coefficient_type, position, context, 5, magnitude == 4);
            }
            return Ok(());
        }
        let at_least_eleven = magnitude >= 11;
        self.record_node(coefficient_type, position, context, 6, at_least_eleven);
        if !at_least_eleven {
            self.record_node(coefficient_type, position, context, 7, magnitude >= 7);
            return Ok(());
        }
        let category = match magnitude {
            11..=18 => 0,
            19..=34 => 1,
            35..=66 => 2,
            67..=2_114 => 3,
            _ => return Err(Vp8EncodeError::FirstPartitionTooLarge),
        };
        let high = category / 2;
        let low = category % 2;
        self.record_node(coefficient_type, position, context, 8, high != 0);
        self.record_node(coefficient_type, position, context, 9 + high, low != 0);
        Ok(())
    }

    fn record_node(
        &mut self,
        coefficient_type: CoefficientBlockType,
        position: usize,
        context: usize,
        node: usize,
        bit: bool,
    ) {
        self.nodes[coefficient_type as usize][COEFFICIENT_BANDS[position]][context][node]
            [usize::from(bit)] += 1;
    }

    fn adapted_probabilities(&self) -> CoefficientProbabilities {
        let mut probabilities = CoefficientProbabilities::default();
        for (coefficient_type, bands) in self.nodes.iter().enumerate() {
            for (band, contexts) in bands.iter().enumerate() {
                for (context, nodes) in contexts.iter().enumerate() {
                    for (node, counts) in nodes.iter().enumerate() {
                        let total = counts[0] + counts[1];
                        if total == 0 {
                            continue;
                        }
                        let candidate = (255 - counts[1] * 255 / total) as u8;
                        let current = COEFFICIENT_DEFAULTS[coefficient_type][band][context][node];
                        let update = COEFFICIENT_UPDATE_PROBABILITIES[coefficient_type][band]
                            [context][node];
                        let current_cost = branch_cost(*counts, current) + bit_cost(false, update);
                        let candidate_cost = branch_cost(*counts, candidate)
                            + bit_cost(true, update)
                            + 8 * 256;
                        if candidate != current && candidate_cost < current_cost {
                            probabilities.values[coefficient_type][band][context][node] = candidate;
                        }
                    }
                }
            }
        }
        probabilities
    }
}

fn branch_cost(counts: [u64; 2], probability: u8) -> u64 {
    counts[0] * bit_cost(false, probability) + counts[1] * bit_cost(true, probability)
}

fn bit_cost(bit: bool, probability: u8) -> u64 {
    // A deterministic Q8 log2 approximation is sufficient to shortlist
    // profitable node updates. The complete encoded partition byte lengths
    // remain the authoritative no-expansion decision.
    let likelihood = if bit { 255 - probability } else { probability }.max(1);
    let likelihood = u32::from(likelihood);
    let exponent = 31 - likelihood.leading_zeros();
    let unit = 1_u32 << exponent;
    let fraction = ((likelihood - unit) << 8) / unit;
    u64::from((8 - exponent) * 256 - fraction)
}

fn coefficient_cost(values: impl Iterator<Item = i16>) -> u64 {
    values
        .map(|value| u64::from(value.unsigned_abs()))
        .sum()
}

fn luma_coefficient_cost(y2: [i16; 16], luma: [[i16; 16]; 16]) -> u64 {
    coefficient_cost(y2.into_iter().chain(luma.into_iter().flatten()))
}

fn chroma_coefficient_cost(u: [[i16; 16]; 4], v: [[i16; 16]; 4]) -> u64 {
    coefficient_cost(u.into_iter().flatten().chain(v.into_iter().flatten()))
}

fn luma_distortion(y: &[u8], y_stride: usize, pixels: &[u8; 256]) -> u64 {
    let mut score = 0_u64;
    for row in 0..16 {
        for column in 0..16 {
            score += u64::from(y[row * y_stride + column].abs_diff(pixels[row * 16 + column]));
        }
    }
    score
}

fn chroma_distortion(
    u: &[u8],
    v: &[u8],
    uv_stride: usize,
    pixels_u: &[u8; 64],
    pixels_v: &[u8; 64],
) -> u64 {
    let mut score = 0_u64;
    for row in 0..8 {
        for column in 0..8 {
            score += u64::from(u[row * uv_stride + column].abs_diff(pixels_u[row * 8 + column]));
            score += u64::from(v[row * uv_stride + column].abs_diff(pixels_v[row * 8 + column]));
        }
    }
    score
}

fn dc_macroblock_residuals(coefficients: Vp8DcMacroblockCoefficients) -> MacroblockResiduals {
    MacroblockResiduals {
        y2: Some(decoded_coefficients(coefficients.y2)),
        luma: coefficients.luma.map(decoded_coefficients),
        u: coefficients.u.map(decoded_coefficients),
        v: coefficients.v.map(decoded_coefficients),
        non_zero_y: 0,
        non_zero_uv: 0,
    }
}

fn decoded_coefficients(values: [i16; 16]) -> DecodedCoefficients {
    let non_zero = values.iter().filter(|&&value| value != 0).count() as u8;
    DecodedCoefficients {
        values,
        // Reconstruction consumes the values directly. The token partition
        // reader owns the more detailed entropy-position bookkeeping.
        end: if non_zero == 0 { 0 } else { 16 },
        non_zero,
    }
}

fn quantization_matrix(quantizer: u8) -> DequantizationMatrix {
    derive_dequantization(
        QuantizationHeader {
            base_index: quantizer,
            y1_dc_delta: 0,
            y2_dc_delta: 0,
            y2_ac_delta: 0,
            uv_dc_delta: 0,
            uv_ac_delta: 0,
        },
        &SegmentHeader {
            enabled: false,
            update_map: false,
            absolute_delta: false,
            quantizer: [0; 4],
            filter_strength: [0; 4],
            probabilities: [255; 3],
        },
    )[0]
}

fn assemble_key_frame(
    width: u32,
    height: u32,
    first_partition: Vec<u8>,
    token_partition: Vec<u8>,
) -> Result<Vec<u8>, Vp8EncodeError> {
    let first_partition_len =
        u32::try_from(first_partition.len()).map_err(|_| Vp8EncodeError::FirstPartitionTooLarge)?;
    if first_partition_len > 0x7ffff {
        return Err(Vp8EncodeError::FirstPartitionTooLarge);
    }
    let capacity = KEY_FRAME_HEADER_LEN
        .checked_add(first_partition.len())
        .and_then(|size| size.checked_add(token_partition.len()))
        .ok_or(Vp8EncodeError::AllocationFailed)?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(capacity)
        .map_err(|_| Vp8EncodeError::AllocationFailed)?;
    let frame_tag = (first_partition_len << 5) | (1 << 4); // Key frame, version 0, show frame.
    output.extend_from_slice(&frame_tag.to_le_bytes()[..3]);
    output.extend_from_slice(&KEY_FRAME_START_CODE);
    output.extend_from_slice(&(width as u16).to_le_bytes());
    output.extend_from_slice(&(height as u16).to_le_bytes());
    output.extend_from_slice(&first_partition);
    output.extend_from_slice(&token_partition);
    Ok(output)
}

/// Converts straight RGBA8 into edge-replicated, macroblock-aligned VP8 YUV420.
///
/// Alpha is retained by the caller's WebP container policy; the VP8 luma and
/// chroma planes are derived from the straight RGB channels only.
pub fn rgba_to_yuv420(
    width: u32,
    height: u32,
    rgba: &[u8],
) -> Result<Vp8SourceYuv, Vp8EncodeError> {
    if width == 0 || height == 0 || width > 0x3fff || height > 0x3fff {
        return Err(Vp8EncodeError::InvalidDimensions);
    }
    let expected = usize::try_from(u64::from(width) * u64::from(height))
        .ok()
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or(Vp8EncodeError::AllocationFailed)?;
    if rgba.len() != expected {
        return Err(Vp8EncodeError::InvalidRgbaLength);
    }
    let macroblock_width =
        usize::try_from(width.div_ceil(16)).map_err(|_| Vp8EncodeError::InvalidDimensions)?;
    let macroblock_height =
        usize::try_from(height.div_ceil(16)).map_err(|_| Vp8EncodeError::InvalidDimensions)?;
    let y_stride = macroblock_width
        .checked_mul(16)
        .ok_or(Vp8EncodeError::AllocationFailed)?;
    let y_height = macroblock_height
        .checked_mul(16)
        .ok_or(Vp8EncodeError::AllocationFailed)?;
    let uv_stride = macroblock_width
        .checked_mul(8)
        .ok_or(Vp8EncodeError::AllocationFailed)?;
    let uv_height = macroblock_height
        .checked_mul(8)
        .ok_or(Vp8EncodeError::AllocationFailed)?;
    let y_len = y_stride
        .checked_mul(y_height)
        .ok_or(Vp8EncodeError::AllocationFailed)?;
    let uv_len = uv_stride
        .checked_mul(uv_height)
        .ok_or(Vp8EncodeError::AllocationFailed)?;
    let mut y = reserve_zeroed(y_len)?;
    let mut u = reserve_zeroed(uv_len)?;
    let mut v = reserve_zeroed(uv_len)?;
    let source_width = usize::try_from(width).map_err(|_| Vp8EncodeError::InvalidDimensions)?;
    let source_height = usize::try_from(height).map_err(|_| Vp8EncodeError::InvalidDimensions)?;
    for row in 0..uv_height {
        for column in 0..uv_stride {
            let mut totals = [0_u16; 3];
            for y_offset in 0..2 {
                for x_offset in 0..2 {
                    let y_row = row * 2 + y_offset;
                    let y_column = column * 2 + x_offset;
                    let [red, green, blue] = rgb_at(
                        rgba,
                        source_width,
                        source_height,
                        y_column,
                        y_row,
                    );
                    y[y_row * y_stride + y_column] = rgb_to_y(red, green, blue);
                    totals[0] += u16::from(red);
                    totals[1] += u16::from(green);
                    totals[2] += u16::from(blue);
                }
            }
            let red = ((totals[0] + 2) / 4) as u8;
            let green = ((totals[1] + 2) / 4) as u8;
            let blue = ((totals[2] + 2) / 4) as u8;
            let index = row * uv_stride + column;
            u[index] = rgb_to_u(red, green, blue);
            v[index] = rgb_to_v(red, green, blue);
        }
    }
    Ok(Vp8SourceYuv {
        width,
        height,
        y_stride,
        uv_stride,
        y,
        u,
        v,
    })
}

/// Transforms and quantizes one DC-predicted 16×16/8×8 macroblock.
///
/// `y`, `u`, and `v` begin at the macroblock's top-left sample. The supplied
/// prediction values are the already-reconstructed DC intra predictions for
/// their respective planes. This keeps prediction ownership in the frame
/// writer while making the transform/Y2 layout independently testable.
pub fn quantize_dc_macroblock(
    y: &[u8],
    y_stride: usize,
    u: &[u8],
    v: &[u8],
    uv_stride: usize,
    prediction: [u8; 3],
    matrix: DequantizationMatrix,
) -> Result<Vp8DcMacroblockCoefficients, Vp8EncodeError> {
    quantize_intra16_macroblock(
        y,
        y_stride,
        u,
        v,
        uv_stride,
        crate::MacroblockPixels {
            y: [prediction[0]; 256],
            u: [prediction[1]; 64],
            v: [prediction[2]; 64],
        },
        matrix,
    )
}

fn quantize_intra16_macroblock(
    y: &[u8],
    y_stride: usize,
    u: &[u8],
    v: &[u8],
    uv_stride: usize,
    prediction: crate::MacroblockPixels,
    matrix: DequantizationMatrix,
) -> Result<Vp8DcMacroblockCoefficients, Vp8EncodeError> {
    if y_stride < 16
        || uv_stride < 8
        || !has_plane_extent(y, y_stride, 16, 16)
        || !has_plane_extent(u, uv_stride, 8, 8)
        || !has_plane_extent(v, uv_stride, 8, 8)
    {
        return Err(Vp8EncodeError::InvalidPlaneLayout);
    }
    let (y2, luma) = quantize_luma_plane(y, y_stride, &prediction.y, matrix);
    Ok(Vp8DcMacroblockCoefficients {
        y2,
        luma,
        u: quantize_chroma_plane(u, uv_stride, &prediction.u, matrix),
        v: quantize_chroma_plane(v, uv_stride, &prediction.v, matrix),
    })
}

fn quantize_luma_plane(
    y: &[u8],
    y_stride: usize,
    prediction: &[u8; 256],
    matrix: DequantizationMatrix,
) -> ([i16; 16], [[i16; 16]; 16]) {
    let mut luma = [[0_i16; 16]; 16];
    let mut luma_dc = [0_i32; 16];
    for block_y in 0..4 {
        for block_x in 0..4 {
            let block = block_y * 4 + block_x;
            let transformed = forward_dct_4x4_i32(residual_block(
                y,
                y_stride,
                block_x * 4,
                block_y * 4,
                prediction,
                16,
            ));
            luma_dc[block] = transformed[0];
            let mut ac_only = transformed;
            ac_only[0] = 0;
            luma[block] = quantize_block(ac_only, matrix.y1_dc, matrix.y1_ac);
        }
    }
    let y2 = quantize_block(forward_wht_4x4_i32(luma_dc), matrix.y2_dc, matrix.y2_ac);
    (y2, luma)
}

fn quantize_chroma_plane(
    plane: &[u8],
    stride: usize,
    prediction: &[u8; 64],
    matrix: DequantizationMatrix,
) -> [[i16; 16]; 4] {
    std::array::from_fn(|block| {
        let block_x = (block % 2) * 4;
        let block_y = (block / 2) * 4;
        quantize_block(
            forward_dct_4x4_i32(residual_block(plane, stride, block_x, block_y, prediction, 8)),
            matrix.uv_dc,
            matrix.uv_ac,
        )
    })
}

fn has_plane_extent(plane: &[u8], stride: usize, width: usize, height: usize) -> bool {
    height
        .checked_sub(1)
        .and_then(|last_row| last_row.checked_mul(stride))
        .and_then(|offset| offset.checked_add(width))
        .is_some_and(|needed| plane.len() >= needed)
}

fn residual_block(
    plane: &[u8],
    stride: usize,
    x: usize,
    y: usize,
    prediction: &[u8],
    prediction_stride: usize,
) -> [i32; 16] {
    std::array::from_fn(|index| {
        let row = y + index / 4;
        let column = x + index % 4;
        i32::from(plane[row * stride + column])
            - i32::from(prediction[row * prediction_stride + column])
    })
}

fn reserve_zeroed(len: usize) -> Result<Vec<u8>, Vp8EncodeError> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(len)
        .map_err(|_| Vp8EncodeError::AllocationFailed)?;
    output.resize(len, 0);
    Ok(output)
}

fn rgb_at(rgba: &[u8], width: usize, height: usize, x: usize, y: usize) -> [u8; 3] {
    let x = x.min(width - 1);
    let y = y.min(height - 1);
    let offset = (y * width + x) * 4;
    [rgba[offset], rgba[offset + 1], rgba[offset + 2]]
}

const fn rgb_to_y(red: u8, green: u8, blue: u8) -> u8 {
    (((66 * red as u32 + 129 * green as u32 + 25 * blue as u32 + 128) >> 8) + 16) as u8
}

const fn rgb_to_u(red: u8, green: u8, blue: u8) -> u8 {
    (((-38 * red as i32 - 74 * green as i32 + 112 * blue as i32 + 128) >> 8) + 128) as u8
}

const fn rgb_to_v(red: u8, green: u8, blue: u8) -> u8 {
    (((112 * red as i32 - 94 * green as i32 - 18 * blue as i32 + 128) >> 8) + 128) as u8
}

fn write_first_partition(
    blocks: &[IntraMacroblock],
    quantizer: u8,
    probabilities: &CoefficientProbabilities,
    skip_probability: Option<u8>,
) -> Result<Vec<u8>, Vp8EncodeError> {
    let mut bits = BoolEncoder::new();
    bits.write_bool(false, 128)?; // WebP YUV color space.
    bits.write_bool(false, 128)?; // Clamp type.
    bits.write_bool(false, 128)?; // Segmentation disabled.
    bits.write_bool(false, 128)?; // Normal loop filter.
    bits.write_literal(0, 6)?; // Filter level.
    bits.write_literal(0, 3)?; // Sharpness.
    bits.write_bool(false, 128)?; // Filter deltas disabled.
    bits.write_literal(0, 2)?; // One token partition.
    bits.write_literal(u32::from(quantizer), 7)?; // Quantizer index.
    for _ in 0..5 {
        bits.write_bool(false, 128)?; // Quantizer delta absent.
    }
    bits.write_bool(false, 128)?; // Refresh entropy probabilities.
    for (coefficient_type, bands) in COEFFICIENT_UPDATE_PROBABILITIES.iter().enumerate() {
        for (band, contexts) in bands.iter().enumerate() {
            for (context, nodes) in contexts.iter().enumerate() {
                for (node, &update_probability) in nodes.iter().enumerate() {
                    let probability = probabilities.values[coefficient_type][band][context][node];
                    let update = probability
                        != COEFFICIENT_DEFAULTS[coefficient_type][band][context][node];
                    bits.write_bool(update, update_probability)?;
                    if update {
                        bits.write_literal(u32::from(probability), 8)?;
                    }
                }
            }
        }
    }
    bits.write_bool(skip_probability.is_some(), 128)?;
    if let Some(probability) = skip_probability {
        bits.write_literal(u32::from(probability), 8)?;
    }
    for &block in blocks {
        if block.segment != 0 {
            return Err(Vp8EncodeError::InvalidPlaneLayout);
        }
        if let Some(probability) = skip_probability {
            bits.write_bool(block.skip, probability)?;
        }
        let LumaMode::Sixteen(luma_mode) = block.luma else {
            return Err(Vp8EncodeError::InvalidPlaneLayout);
        };
        bits.write_bool(true, 145)?; // 16×16 luma mode.
        match luma_mode {
            Intra16Mode::Dc => {
                bits.write_bool(false, 156)?;
                bits.write_bool(false, 163)?;
            }
            Intra16Mode::Vertical => {
                bits.write_bool(false, 156)?;
                bits.write_bool(true, 163)?;
            }
            Intra16Mode::Horizontal => {
                bits.write_bool(true, 156)?;
                bits.write_bool(false, 128)?;
            }
            Intra16Mode::TrueMotion => {
                bits.write_bool(true, 156)?;
                bits.write_bool(true, 128)?;
            }
        }
        match block.chroma {
            ChromaMode::Dc => bits.write_bool(false, 142)?,
            ChromaMode::Vertical => {
                bits.write_bool(true, 142)?;
                bits.write_bool(false, 114)?;
            }
            ChromaMode::Horizontal => {
                bits.write_bool(true, 142)?;
                bits.write_bool(true, 114)?;
                bits.write_bool(false, 183)?;
            }
            ChromaMode::TrueMotion => {
                bits.write_bool(true, 142)?;
                bits.write_bool(true, 114)?;
                bits.write_bool(true, 183)?;
            }
        }
    }
    bits.finish().map_err(Into::into)
}

fn write_zero_token_partition(macroblock_count: usize) -> Result<Vec<u8>, Vp8EncodeError> {
    let mut bits = BoolEncoder::new();
    for _ in 0..macroblock_count {
        write_eob(&mut bits, 1, 0)?; // Y2.
        for _ in 0..16 {
            write_eob(&mut bits, 0, 1)?; // Luma AC-only blocks.
        }
        for _ in 0..8 {
            write_eob(&mut bits, 2, 0)?; // Four U and four V blocks.
        }
    }
    bits.finish().map_err(Into::into)
}

fn collect_coefficient_statistics(
    macroblocks: &[Vp8DcMacroblockCoefficients],
    blocks: &[IntraMacroblock],
    macroblock_width: usize,
    honor_skip: bool,
) -> Result<CoefficientStatistics, Vp8EncodeError> {
    if macroblock_width == 0
        || macroblocks.len() != blocks.len()
        || !macroblocks.len().is_multiple_of(macroblock_width)
    {
        return Err(Vp8EncodeError::InvalidPlaneLayout);
    }
    let mut statistics = CoefficientStatistics::new();
    let mut top_y2 = vec![false; macroblock_width];
    let mut top_luma = vec![[false; 4]; macroblock_width];
    let mut top_u = vec![[false; 2]; macroblock_width];
    let mut top_v = vec![[false; 2]; macroblock_width];
    for (row, block_row) in macroblocks
        .chunks_exact(macroblock_width)
        .zip(blocks.chunks_exact(macroblock_width))
    {
        let mut left_y2 = false;
        let mut left_luma = [false; 4];
        let mut left_u = [false; 2];
        let mut left_v = [false; 2];
        for (column, (coefficients, block)) in row
            .iter()
            .copied()
            .zip(block_row.iter().copied())
            .enumerate()
        {
            if honor_skip && block.skip {
                top_y2[column] = false;
                top_luma[column] = [false; 4];
                top_u[column] = [false; 2];
                top_v[column] = [false; 2];
                left_y2 = false;
                left_luma = [false; 4];
                left_u = [false; 2];
                left_v = [false; 2];
                continue;
            }
            let y2_context = u8::from(top_y2[column]) + u8::from(left_y2);
            statistics.record(
                CoefficientBlockType::LumaDc,
                y2_context,
                0,
                coefficients.y2,
            )?;
            let y2_present = coefficients.y2.iter().any(|&value| value != 0);
            top_y2[column] = y2_present;
            left_y2 = y2_present;
            record_luma_statistics(
                &mut statistics,
                coefficients.luma,
                &mut top_luma[column],
                &mut left_luma,
            )?;
            record_chroma_statistics(
                &mut statistics,
                coefficients.u,
                &mut top_u[column],
                &mut left_u,
            )?;
            record_chroma_statistics(
                &mut statistics,
                coefficients.v,
                &mut top_v[column],
                &mut left_v,
            )?;
        }
    }
    Ok(statistics)
}

fn record_luma_statistics(
    statistics: &mut CoefficientStatistics,
    blocks: [[i16; 16]; 16],
    top: &mut [bool; 4],
    left: &mut [bool; 4],
) -> Result<(), Vp8EncodeError> {
    for row in 0..4 {
        let mut left_block = left[row];
        for column in 0..4 {
            let block = blocks[row * 4 + column];
            let context = u8::from(top[column]) + u8::from(left_block);
            statistics.record(CoefficientBlockType::Luma16Ac, context, 1, block)?;
            let present = block[1..].iter().any(|&value| value != 0);
            top[column] = present;
            left_block = present;
        }
        left[row] = left_block;
    }
    Ok(())
}

fn record_chroma_statistics(
    statistics: &mut CoefficientStatistics,
    blocks: [[i16; 16]; 4],
    top: &mut [bool; 2],
    left: &mut [bool; 2],
) -> Result<(), Vp8EncodeError> {
    for row in 0..2 {
        let mut left_block = left[row];
        for column in 0..2 {
            let block = blocks[row * 2 + column];
            let context = u8::from(top[column]) + u8::from(left_block);
            statistics.record(CoefficientBlockType::ChromaAc, context, 0, block)?;
            let present = block.iter().any(|&value| value != 0);
            top[column] = present;
            left_block = present;
        }
        left[row] = left_block;
    }
    Ok(())
}

fn write_dc_macroblocks_token_partition(
    macroblocks: &[Vp8DcMacroblockCoefficients],
    blocks: &[IntraMacroblock],
    macroblock_width: usize,
    honor_skip: bool,
    probabilities: &CoefficientProbabilities,
) -> Result<Vec<u8>, Vp8EncodeError> {
    if macroblock_width == 0
        || macroblocks.len() != blocks.len()
        || !macroblocks.len().is_multiple_of(macroblock_width)
    {
        return Err(Vp8EncodeError::InvalidPlaneLayout);
    }
    let mut bits = BoolEncoder::new();
    let mut top_y2 = vec![false; macroblock_width];
    let mut top_luma = vec![[false; 4]; macroblock_width];
    let mut top_u = vec![[false; 2]; macroblock_width];
    let mut top_v = vec![[false; 2]; macroblock_width];
    for (row, block_row) in macroblocks
        .chunks_exact(macroblock_width)
        .zip(blocks.chunks_exact(macroblock_width))
    {
        let mut left_y2 = false;
        let mut left_luma = [false; 4];
        let mut left_u = [false; 2];
        let mut left_v = [false; 2];
        for (column, (coefficients, block)) in row
            .iter()
            .copied()
            .zip(block_row.iter().copied())
            .enumerate()
        {
            if honor_skip && block.skip {
                top_y2[column] = false;
                top_luma[column] = [false; 4];
                top_u[column] = [false; 2];
                top_v[column] = [false; 2];
                left_y2 = false;
                left_luma = [false; 4];
                left_u = [false; 2];
                left_v = [false; 2];
                continue;
            }
            let y2_context = u8::from(top_y2[column]) + u8::from(left_y2);
            write_coefficients(
                &mut bits,
                probabilities,
                CoefficientBlockType::LumaDc,
                y2_context,
                0,
                coefficients.y2,
            )?;
            let y2_present = coefficients.y2.iter().any(|&value| value != 0);
            top_y2[column] = y2_present;
            left_y2 = y2_present;
            write_luma_coefficients(
                &mut bits,
                probabilities,
                coefficients.luma,
                &mut top_luma[column],
                &mut left_luma,
            )?;
            write_chroma_coefficients(
                &mut bits,
                probabilities,
                coefficients.u,
                &mut top_u[column],
                &mut left_u,
            )?;
            write_chroma_coefficients(
                &mut bits,
                probabilities,
                coefficients.v,
                &mut top_v[column],
                &mut left_v,
            )?;
        }
    }
    bits.finish().map_err(Into::into)
}

fn write_luma_coefficients(
    bits: &mut BoolEncoder,
    probabilities: &CoefficientProbabilities,
    blocks: [[i16; 16]; 16],
    top: &mut [bool; 4],
    left: &mut [bool; 4],
) -> Result<(), Vp8EncodeError> {
    for row in 0..4 {
        let mut left_block = left[row];
        for column in 0..4 {
            let block = blocks[row * 4 + column];
            let context = u8::from(top[column]) + u8::from(left_block);
            write_coefficients(
                bits,
                probabilities,
                CoefficientBlockType::Luma16Ac,
                context,
                1,
                block,
            )?;
            let present = block[1..].iter().any(|&value| value != 0);
            top[column] = present;
            left_block = present;
        }
        left[row] = left_block;
    }
    Ok(())
}

fn write_chroma_coefficients(
    bits: &mut BoolEncoder,
    probabilities: &CoefficientProbabilities,
    blocks: [[i16; 16]; 4],
    top: &mut [bool; 2],
    left: &mut [bool; 2],
) -> Result<(), Vp8EncodeError> {
    for row in 0..2 {
        let mut left_block = left[row];
        for column in 0..2 {
            let block = blocks[row * 2 + column];
            let context = u8::from(top[column]) + u8::from(left_block);
            write_coefficients(
                bits,
                probabilities,
                CoefficientBlockType::ChromaAc,
                context,
                0,
                block,
            )?;
            let present = block.iter().any(|&value| value != 0);
            top[column] = present;
            left_block = present;
        }
        left[row] = left_block;
    }
    Ok(())
}

fn write_coefficients(
    bits: &mut BoolEncoder,
    probabilities: &CoefficientProbabilities,
    coefficient_type: CoefficientBlockType,
    context: u8,
    start: u8,
    values: [i16; 16],
) -> Result<(), Vp8EncodeError> {
    encode_coefficients(bits, probabilities, coefficient_type, context, start, values).map_err(
        |error| match error {
            CoefficientEncodeError::AllocationFailed => Vp8EncodeError::AllocationFailed,
            CoefficientEncodeError::InvalidParameter | CoefficientEncodeError::CoefficientOutOfRange => {
                Vp8EncodeError::FirstPartitionTooLarge
            }
        },
    )
}

fn write_eob(
    bits: &mut BoolEncoder,
    coefficient_type: usize,
    position: usize,
) -> Result<(), Vp8EncodeError> {
    let probability = COEFFICIENT_DEFAULTS[coefficient_type][coefficient_band(position)][0][0];
    bits.write_bool(false, probability).map_err(Into::into)
}

const fn coefficient_band(position: usize) -> usize {
    const BANDS: [usize; 17] = [0, 1, 2, 3, 6, 4, 5, 6, 6, 6, 6, 6, 6, 6, 6, 7, 0];
    BANDS[position]
}

#[cfg(test)]
#[path = "encoder_tests.rs"]
mod tests;
