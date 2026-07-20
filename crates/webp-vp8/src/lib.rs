#![forbid(unsafe_code)]
//! VP8 lossy WebP decoder primitives.
//!
//! This M2 foundation parses the complete uncompressed VP8 key-frame header
//! and validates the first-partition boundary before any entropy state or
//! macroblock storage is allocated.  Pixel decoding is intentionally kept out
//! of this crate's first slice.

use webp_core::{DecodeError, DecodeErrorKind, DecodeLimits, WorkBudget};

mod coefficients;

use coefficients::{COEFFICIENT_DEFAULTS, COEFFICIENT_UPDATE_PROBABILITIES};

const FRAME_TAG_LEN: usize = 3;
const KEY_FRAME_HEADER_LEN: usize = 10;
const KEY_FRAME_START_CODE: [u8; 3] = [0x9d, 0x01, 0x2a];

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
    limits.check_input_len(payload.len())?;
    let first_partition_end = FRAME_TAG_LEN
        .checked_add(frame.first_partition_len)
        .ok_or_else(|| {
            DecodeError::at(
                DecodeErrorKind::InvalidBitstream,
                FRAME_TAG_LEN,
                "VP8 first partition end overflows",
            )
        })?;
    if first_partition_end > payload.len() || first_partition_end < KEY_FRAME_HEADER_LEN {
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

    Ok(PartitionLayout {
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
    })
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
/// `first_partition_len` counts bytes immediately following the three-byte
/// frame tag.  Its end is validated against the supplied VP8 payload but its
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
    if first_partition_len > payload.len() - FRAME_TAG_LEN {
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
        writer.write_literal(0, 8); // Leave structural fields away from EOF.
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
        let payload = key_frame(0x800d, 0xc009, 3, true, 7);
        let header = parse_riff_payload(&payload, Some((13, 9)), &DecodeLimits::default()).unwrap();
        assert_eq!(header.width, 13);
        assert_eq!(header.height, 9);
        assert_eq!(header.version, 3);
        assert_eq!(header.first_partition_len, 7);
        assert_eq!(header.horizontal_scale, 2);
        assert_eq!(header.vertical_scale, 3);
    }

    #[test]
    fn rejects_all_fixed_header_truncations() {
        let payload = key_frame(1, 1, 0, true, 7);
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
        let mut inter = key_frame(1, 1, 0, true, 7);
        inter[0] |= 1;
        assert_eq!(
            parse_riff_payload(&inter, None, &limits)
                .unwrap_err()
                .kind(),
            DecodeErrorKind::UnsupportedFeature
        );

        let invisible = key_frame(1, 1, 0, false, 7);
        assert_eq!(
            parse_riff_payload(&invisible, None, &limits)
                .unwrap_err()
                .kind(),
            DecodeErrorKind::InvalidBitstream
        );

        let unsupported_version = key_frame(1, 1, 4, true, 7);
        assert_eq!(
            parse_riff_payload(&unsupported_version, None, &limits)
                .unwrap_err()
                .kind(),
            DecodeErrorKind::InvalidBitstream
        );

        let mut bad_signature = key_frame(1, 1, 0, true, 7);
        bad_signature[5] ^= 1;
        assert_eq!(
            parse_riff_payload(&bad_signature, None, &limits)
                .unwrap_err()
                .kind(),
            DecodeErrorKind::InvalidBitstream
        );

        let zero_width = key_frame(0, 1, 0, true, 7);
        assert_eq!(
            parse_riff_payload(&zero_width, None, &limits)
                .unwrap_err()
                .kind(),
            DecodeErrorKind::InvalidBitstream
        );

        let partition_past_end = key_frame(1, 1, 0, true, 8);
        assert_eq!(
            parse_riff_payload(&partition_past_end, None, &limits)
                .unwrap_err()
                .kind(),
            DecodeErrorKind::UnexpectedEof
        );

        let valid = key_frame(1, 1, 0, true, 7);
        assert_eq!(
            parse_riff_payload(&valid, Some((2, 1)), &limits)
                .unwrap_err()
                .kind(),
            DecodeErrorKind::InvalidContainer
        );
    }

    #[test]
    fn enforces_image_limits_before_decoder_state_is_created() {
        let payload = key_frame(8, 1, 0, true, 7);
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
        let mut partition_zero = writer.finish();
        partition_zero.extend_from_slice(&[0; 8]);

        let mut payload = key_frame(3, 5, 0, true, 7 + partition_zero.len() as u32).to_vec();
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
        let partition_zero = writer.finish();
        let mut payload = key_frame(1, 1, 0, true, 7 + partition_zero.len() as u32).to_vec();
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
            let partition_zero = writer.finish();
            let partition_count = 1_usize << partition_bits;
            let mut payload = key_frame(1, 1, 0, true, 7 + partition_zero.len() as u32).to_vec();
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
}
