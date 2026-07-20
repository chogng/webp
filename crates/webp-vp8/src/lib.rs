#![forbid(unsafe_code)]
//! VP8 lossy WebP decoder primitives.
//!
//! This M2 foundation parses the complete uncompressed VP8 key-frame header
//! and validates the first-partition boundary before any entropy state or
//! macroblock storage is allocated.  Pixel decoding is intentionally kept out
//! of this crate's first slice.

use webp_core::{DecodeError, DecodeErrorKind, DecodeLimits, WorkBudget};

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
        let bytes = writer.finish();
        let mut decoder = BoolDecoder::new(&bytes, &DecodeLimits::default()).unwrap();
        assert_eq!(decoder.read_literal(5), Ok(0b10110));
        assert_eq!(decoder.read_literal(16), Ok(0x1234));
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
}
