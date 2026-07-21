//! VP8 key-frame and first-partition parsing.
//!
//! This layer validates the uncompressed key-frame prefix and exposes
//! zero-copy coefficient-token partitions. It deliberately stops before
//! macroblock entropy decoding and pixel reconstruction.

use webp_core::{DecodeError, DecodeErrorKind, DecodeLimits};

use crate::coefficients::COEFFICIENT_UPDATE_PROBABILITIES;
use crate::{BoolDecoder, CoefficientProbabilities, QuantizationHeader};

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

const FRAME_TAG_LEN: usize = 3;
pub(crate) const KEY_FRAME_HEADER_LEN: usize = 10;
pub(crate) const KEY_FRAME_START_CODE: [u8; 3] = [0x9d, 0x01, 0x2a];

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
pub(crate) fn parse_partition_layout_with_mode_decoder<'a>(
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
