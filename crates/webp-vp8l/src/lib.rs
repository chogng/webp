#![forbid(unsafe_code)]
//! VP8L lossless WebP primitives.
//!
//! This first slice deliberately validates only the fixed VP8L header.  It
//! performs no entropy decoding and never allocates pixel storage.  Callers
//! handling a RIFF file should first validate its chunk framing, then pass the
//! `VP8L` payload to [`parse_riff_payload`].

use webp_core::{BitReader, DecodeError, DecodeErrorKind, DecodeLimits, checked_chunk_end};

/// The byte value at the beginning of every VP8L bitstream.
pub const SIGNATURE: u8 = 0x2f;

/// The number of bytes occupied by the fixed VP8L header.
pub const HEADER_LEN: usize = 5;

/// The number of bytes in a RIFF chunk header.
pub const RIFF_CHUNK_HEADER_LEN: usize = 8;

/// The largest dimension representable by the VP8L 14-bit, minus-one field.
pub const MAX_DIMENSION: u32 = 1 << 14;

/// Fixed information at the beginning of a VP8L bitstream.
///
/// `alpha_is_used` is a coding hint only.  A decoder must derive actual alpha
/// values from the later pixel data rather than treating this flag as a pixel
/// semantic.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Vp8lHeader {
    pub width: u32,
    pub height: u32,
    pub alpha_is_used: bool,
    pub version: u8,
}

/// Parses a standalone VP8L bitstream header.
///
/// `data` begins with the VP8L signature byte, not a RIFF chunk header.  The
/// function accepts bytes following the fixed header because they belong to
/// the VP8L entropy stream, which is parsed by later decoder stages.
///
/// No pixel or table allocation is attempted before [`DecodeLimits`] are
/// applied.
pub fn parse_header(data: &[u8], limits: &DecodeLimits) -> Result<Vp8lHeader, DecodeError> {
    limits.check_input_len(data.len())?;

    let mut bits = BitReader::new(data);
    let signature = bits.read_bits(8)? as u8;
    if signature != SIGNATURE {
        return Err(DecodeError::at(
            DecodeErrorKind::InvalidBitstream,
            0,
            "invalid VP8L signature",
        ));
    }

    // VP8L bits are least-significant-bit first.  The reader makes the first
    // input bit bit 0 of the returned value, exactly matching ReadBits(n).
    let width = bits.read_bits(14)? + 1;
    let height = bits.read_bits(14)? + 1;
    let alpha_is_used = bits.read_bit()?;
    let version = bits.read_bits(3)? as u8;
    if version != 0 {
        return Err(DecodeError::at(
            DecodeErrorKind::InvalidBitstream,
            4,
            "unsupported VP8L version",
        ));
    }

    // This happens before any future entropy table or pixel allocation.
    limits.check_image(width, height)?;

    Ok(Vp8lHeader {
        width,
        height,
        alpha_is_used,
        version,
    })
}

/// Parses a `VP8L` RIFF chunk payload and checks its optional `VP8X` canvas.
///
/// The caller must pass the exact payload bytes after RIFF chunk framing has
/// been validated.  `canvas` is `Some` only for an extended (`VP8X`) static
/// layout; its dimensions must exactly match the embedded VP8L header.
pub fn parse_riff_payload(
    payload: &[u8],
    canvas: Option<(u32, u32)>,
    limits: &DecodeLimits,
) -> Result<Vp8lHeader, DecodeError> {
    let header = parse_header(payload, limits)?;
    if let Some((canvas_width, canvas_height)) = canvas
        && (header.width != canvas_width || header.height != canvas_height)
    {
        return Err(DecodeError::at(
            DecodeErrorKind::InvalidContainer,
            0,
            "VP8X canvas does not match VP8L dimensions",
        ));
    }
    Ok(header)
}

/// Parses one complete, RIFF-framed `VP8L` chunk.
///
/// `chunk` starts with the four-byte `VP8L` FourCC and includes its little
/// endian length field, payload, and any required RIFF padding byte.  This is
/// useful for callers which have isolated a chunk but still need to verify its
/// declared length before handing its payload to the VP8L decoder.
pub fn parse_riff_chunk(
    chunk: &[u8],
    canvas: Option<(u32, u32)>,
    limits: &DecodeLimits,
) -> Result<Vp8lHeader, DecodeError> {
    limits.check_input_len(chunk.len())?;
    if chunk.len() < RIFF_CHUNK_HEADER_LEN {
        return Err(DecodeError::at(
            DecodeErrorKind::UnexpectedEof,
            chunk.len(),
            "truncated VP8L RIFF chunk header",
        ));
    }
    if chunk[..4] != *b"VP8L" {
        return Err(DecodeError::at(
            DecodeErrorKind::InvalidContainer,
            0,
            "expected VP8L RIFF chunk",
        ));
    }
    let payload_len = u32::from_le_bytes(chunk[4..8].try_into().map_err(|_| {
        DecodeError::at(
            DecodeErrorKind::UnexpectedEof,
            4,
            "truncated VP8L RIFF chunk length",
        )
    })?);
    let chunk_end = checked_chunk_end(0, payload_len, chunk.len())?;
    if chunk_end != chunk.len() {
        return Err(DecodeError::at(
            DecodeErrorKind::InvalidContainer,
            chunk_end,
            "VP8L RIFF chunk has trailing bytes",
        ));
    }
    let payload_len = usize::try_from(payload_len).map_err(|_| {
        DecodeError::at(
            DecodeErrorKind::InvalidContainer,
            4,
            "VP8L RIFF chunk length does not fit usize",
        )
    })?;
    let payload = &chunk[RIFF_CHUNK_HEADER_LEN..RIFF_CHUNK_HEADER_LEN + payload_len];
    parse_riff_payload(payload, canvas, limits)
}

#[cfg(test)]
mod tests {
    use super::*;
    use webp_core::BitWriter;

    fn limits() -> DecodeLimits {
        DecodeLimits::default()
    }

    fn header(width: u32, height: u32, alpha: bool, version: u8) -> Vec<u8> {
        assert!((1..=MAX_DIMENSION).contains(&width));
        assert!((1..=MAX_DIMENSION).contains(&height));
        assert!(version < 8);
        let mut writer = BitWriter::new();
        writer.write_bits(u32::from(SIGNATURE), 8).unwrap();
        writer.write_bits(width - 1, 14).unwrap();
        writer.write_bits(height - 1, 14).unwrap();
        writer.write_bits(u32::from(alpha), 1).unwrap();
        writer.write_bits(u32::from(version), 3).unwrap();
        assert_eq!(writer.as_bytes().len(), HEADER_LEN);
        writer.into_bytes()
    }

    #[test]
    fn parses_lsb_header_fields() {
        let bytes = header(0x1234, 0x0234, true, 0);
        assert_eq!(
            parse_header(&bytes, &limits()).unwrap(),
            Vp8lHeader {
                width: 0x1234,
                height: 0x0234,
                alpha_is_used: true,
                version: 0,
            }
        );
    }

    #[test]
    fn accepts_minimum_and_syntax_maximum_dimensions() {
        assert_eq!(
            parse_header(&header(1, 1, false, 0), &limits()).unwrap(),
            Vp8lHeader {
                width: 1,
                height: 1,
                alpha_is_used: false,
                version: 0,
            }
        );
        let max = parse_header(&header(MAX_DIMENSION, MAX_DIMENSION, true, 0), &limits()).unwrap();
        assert_eq!((max.width, max.height), (MAX_DIMENSION, MAX_DIMENSION));
    }

    #[test]
    fn every_signature_bit_is_checked() {
        for bit in 0..8 {
            let mut bytes = header(1, 1, false, 0);
            bytes[0] ^= 1 << bit;
            let error = parse_header(&bytes, &limits()).unwrap_err();
            assert_eq!(error.kind(), DecodeErrorKind::InvalidBitstream);
            assert_eq!(error.offset(), Some(0));
        }
    }

    #[test]
    fn rejects_nonzero_version() {
        for version in 1..8 {
            let error = parse_header(&header(1, 1, false, version), &limits()).unwrap_err();
            assert_eq!(error.kind(), DecodeErrorKind::InvalidBitstream);
            assert_eq!(error.offset(), Some(4));
        }
    }

    #[test]
    fn every_header_truncation_is_eof() {
        let bytes = header(42, 19, false, 0);
        for length in 0..HEADER_LEN {
            let error = parse_header(&bytes[..length], &limits()).unwrap_err();
            assert_eq!(
                error.kind(),
                DecodeErrorKind::UnexpectedEof,
                "length {length}"
            );
        }
    }

    #[test]
    fn limits_apply_before_any_pixel_allocation() {
        let configured = DecodeLimits {
            max_width: MAX_DIMENSION,
            max_height: MAX_DIMENSION,
            max_pixels: 3,
            ..DecodeLimits::default()
        };
        let error = parse_header(&header(2, 2, false, 0), &configured).unwrap_err();
        assert_eq!(error.kind(), DecodeErrorKind::LimitExceeded);
    }

    #[test]
    fn input_byte_limit_is_checked_before_reading() {
        let configured = DecodeLimits {
            max_input_bytes: HEADER_LEN - 1,
            ..DecodeLimits::default()
        };
        let error = parse_header(&header(1, 1, false, 0), &configured).unwrap_err();
        assert_eq!(error.kind(), DecodeErrorKind::LimitExceeded);
    }

    #[test]
    fn riff_payload_validates_canvas_consistency() {
        let bytes = header(7, 9, false, 0);
        assert_eq!(
            parse_riff_payload(&bytes, Some((7, 9)), &limits())
                .unwrap()
                .width,
            7
        );
        let error = parse_riff_payload(&bytes, Some((8, 9)), &limits()).unwrap_err();
        assert_eq!(error.kind(), DecodeErrorKind::InvalidContainer);
    }

    #[test]
    fn riff_chunk_validates_declared_payload_length() {
        let payload = header(7, 9, false, 0);
        let mut chunk = b"VP8L".to_vec();
        chunk.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        chunk.extend_from_slice(&payload);
        chunk.push(0); // VP8L header length is odd, so RIFF requires padding.
        assert_eq!(parse_riff_chunk(&chunk, None, &limits()).unwrap().height, 9);

        let mut truncated = chunk.clone();
        truncated.pop();
        assert_eq!(
            parse_riff_chunk(&truncated, None, &limits())
                .unwrap_err()
                .kind(),
            DecodeErrorKind::UnexpectedEof
        );

        chunk.push(0);
        assert_eq!(
            parse_riff_chunk(&chunk, None, &limits())
                .unwrap_err()
                .kind(),
            DecodeErrorKind::InvalidContainer
        );
    }
}
