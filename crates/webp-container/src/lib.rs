#![forbid(unsafe_code)]
//! Safe, zero-copy parsing of the WebP RIFF container.
//!
//! This crate deliberately stops at the container boundary.  It validates RIFF
//! lengths and chunk framing, exposes unknown chunks unchanged, and decodes the
//! small `VP8X` header without attempting to decode VP8 or VP8L payloads.

use webp_core::{
    CompatibilityProfile, DecodeError, DecodeErrorKind, DecodeLimits, checked_chunk_end,
};

const RIFF_HEADER_LEN: usize = 12;
const CHUNK_HEADER_LEN: usize = 8;

/// A four-byte chunk identifier.  It is intentionally byte based: `FourCC`s are
/// case sensitive and are not necessarily UTF-8 text.
pub type FourCc = [u8; 4];

pub const VP8: FourCc = *b"VP8 ";
pub const VP8L: FourCc = *b"VP8L";
pub const VP8X: FourCc = *b"VP8X";
pub const ALPH: FourCc = *b"ALPH";
pub const ICCP: FourCc = *b"ICCP";
pub const EXIF: FourCc = *b"EXIF";
pub const XMP: FourCc = *b"XMP ";
pub const ANIM: FourCc = *b"ANIM";
pub const ANMF: FourCc = *b"ANMF";

/// A parsed RIFF WebP file.  Payloads borrow from the supplied input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Container<'a> {
    chunks: Vec<Chunk<'a>>,
    vp8x: Option<Vp8x>,
    /// Bytes outside the declared RIFF length.  This is only populated in the
    /// compatible profile; strict parsing rejects such input.
    trailing: &'a [u8],
}

impl<'a> Container<'a> {
    #[must_use]
    pub fn chunks(&self) -> &[Chunk<'a>] {
        &self.chunks
    }

    #[must_use]
    pub fn vp8x(&self) -> Option<Vp8x> {
        self.vp8x
    }

    #[must_use]
    pub fn trailing(&self) -> &'a [u8] {
        self.trailing
    }

    /// Returns metadata as raw bytes.  Metadata is never interpreted as text.
    #[must_use]
    pub fn metadata(&self) -> Metadata<'a> {
        Metadata {
            iccp: first_payload(&self.chunks, ICCP),
            exif: first_payload(&self.chunks, EXIF),
            xmp: first_payload(&self.chunks, XMP),
        }
    }

    /// Iterates chunks which have no meaning to this version of the parser,
    /// retaining their original order and byte contents.
    pub fn unknown_chunks(&self) -> impl Iterator<Item = &Chunk<'a>> {
        self.chunks.iter().filter(|chunk| !is_known(chunk.fourcc))
    }
}

/// One top-level RIFF chunk, including the original padding byte when present.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Chunk<'a> {
    pub fourcc: FourCc,
    pub payload: &'a [u8],
    pub padding: Option<u8>,
    /// Byte offset of the chunk `FourCC` from the beginning of the input.
    pub offset: usize,
}

impl Chunk<'_> {
    #[must_use]
    pub fn is_known(&self) -> bool {
        is_known(self.fourcc)
    }
}

/// Parsed contents of the fixed-size `VP8X` chunk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Vp8x {
    pub flags: Vp8xFlags,
    pub canvas_width: u32,
    pub canvas_height: u32,
}

/// Feature flags declared by `VP8X`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Vp8xFlags(u8);

impl Vp8xFlags {
    const ICCP: u8 = 1 << 5;
    const ALPHA: u8 = 1 << 4;
    const EXIF: u8 = 1 << 3;
    const XMP: u8 = 1 << 2;
    const ANIMATION: u8 = 1 << 1;
    const RESERVED: u8 = (1 << 7) | (1 << 6) | 1;

    #[must_use]
    pub fn iccp(self) -> bool {
        self.0 & Self::ICCP != 0
    }
    #[must_use]
    pub fn alpha(self) -> bool {
        self.0 & Self::ALPHA != 0
    }
    #[must_use]
    pub fn exif(self) -> bool {
        self.0 & Self::EXIF != 0
    }
    #[must_use]
    pub fn xmp(self) -> bool {
        self.0 & Self::XMP != 0
    }
    #[must_use]
    pub fn animation(self) -> bool {
        self.0 & Self::ANIMATION != 0
    }
    #[must_use]
    pub fn reserved_bits(self) -> u8 {
        self.0 & Self::RESERVED
    }
    #[must_use]
    pub fn bits(self) -> u8 {
        self.0
    }
}

/// Borrowed raw metadata selected from the first chunk of each type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Metadata<'a> {
    pub iccp: Option<&'a [u8]>,
    pub exif: Option<&'a [u8]>,
    pub xmp: Option<&'a [u8]>,
}

/// Parses a complete WebP RIFF container.
///
/// `SpecStrict` rejects non-zero chunk padding, RIFF trailing bytes, malformed
/// `VP8X`, duplicate singleton chunks, and inconsistent `VP8X` feature flags.
/// `LibwebpCompatible` keeps parsing those recoverable container quirks while
/// still enforcing all byte boundaries and resource limits.
///
/// # Errors
///
/// Returns an error for invalid magic, incomplete or overflowing RIFF/chunk
/// boundaries, limit violations, and profile-specific layout violations.
#[allow(clippy::too_many_lines)] // Keep the linear parser's boundary checks adjacent.
pub fn parse<'a>(
    data: &'a [u8],
    profile: CompatibilityProfile,
    limits: &DecodeLimits,
) -> Result<Container<'a>, DecodeError> {
    if data.len() > limits.max_input_bytes {
        return Err(error(
            DecodeErrorKind::LimitExceeded,
            0,
            "input exceeds max_input_bytes",
        ));
    }
    if data.len() < RIFF_HEADER_LEN {
        return Err(error(
            DecodeErrorKind::UnexpectedEof,
            data.len(),
            "truncated RIFF header",
        ));
    }
    if data[..4] != *b"RIFF" || data[8..12] != *b"WEBP" {
        return Err(error(
            DecodeErrorKind::InvalidContainer,
            0,
            "missing RIFF/WEBP magic",
        ));
    }

    let declared = read_u32(&data[4..8])?;
    if declared < 4 {
        return Err(error(
            DecodeErrorKind::InvalidContainer,
            4,
            "RIFF size excludes WEBP form type",
        ));
    }
    let container_end = 8usize
        .checked_add(declared as usize)
        .ok_or_else(|| error(DecodeErrorKind::InvalidContainer, 4, "RIFF size overflow"))?;
    if container_end > data.len() {
        return Err(error(
            DecodeErrorKind::UnexpectedEof,
            data.len(),
            "RIFF body is truncated",
        ));
    }
    if profile == CompatibilityProfile::SpecStrict && container_end != data.len() {
        return Err(error(
            DecodeErrorKind::InvalidContainer,
            container_end,
            "bytes trail declared RIFF body",
        ));
    }

    let mut chunks = Vec::new();
    let mut offset = RIFF_HEADER_LEN;
    let mut vp8x = None;
    let mut metadata_bytes = 0usize;
    while offset < container_end {
        if container_end - offset < CHUNK_HEADER_LEN {
            return Err(error(
                DecodeErrorKind::UnexpectedEof,
                offset,
                "truncated chunk header",
            ));
        }
        let fourcc = read_fourcc(&data[offset..offset + 4])?;
        let size = read_u32(&data[offset + 4..offset + 8])?;
        // checked_chunk_end includes the optional RIFF alignment byte and is
        // the single arithmetic boundary authority shared with the core crate.
        let next = checked_chunk_end(offset, size, container_end)?;
        let pad_len = (size & 1) as usize;
        let payload_end = next - pad_len;
        let payload = &data[offset + CHUNK_HEADER_LEN..payload_end];
        let padding = (pad_len == 1).then(|| data[payload_end]);
        if profile == CompatibilityProfile::SpecStrict && padding.is_some_and(|byte| byte != 0) {
            return Err(error(
                DecodeErrorKind::InvalidContainer,
                payload_end,
                "non-zero RIFF padding",
            ));
        }

        if matches!(fourcc, ICCP | EXIF | XMP) {
            metadata_bytes = metadata_bytes.checked_add(payload.len()).ok_or_else(|| {
                error(
                    DecodeErrorKind::LimitExceeded,
                    offset,
                    "metadata size overflow",
                )
            })?;
            if metadata_bytes > limits.max_metadata_bytes {
                return Err(error(
                    DecodeErrorKind::LimitExceeded,
                    offset,
                    "metadata exceeds max_metadata_bytes",
                ));
            }
        }

        if fourcc == VP8X {
            if vp8x.is_some() && profile == CompatibilityProfile::SpecStrict {
                return Err(error(
                    DecodeErrorKind::InvalidContainer,
                    offset,
                    "duplicate VP8X chunk",
                ));
            }
            let parsed = parse_vp8x(payload, profile, limits, offset + CHUNK_HEADER_LEN)?;
            if vp8x.is_none() {
                vp8x = Some(parsed);
            }
        }
        chunks.push(Chunk {
            fourcc,
            payload,
            padding,
            offset,
        });
        offset = next;
    }

    if profile == CompatibilityProfile::SpecStrict {
        validate_strict_layout(&chunks, vp8x)?;
    }
    Ok(Container {
        chunks,
        vp8x,
        trailing: &data[container_end..],
    })
}

fn parse_vp8x(
    payload: &[u8],
    profile: CompatibilityProfile,
    limits: &DecodeLimits,
    payload_offset: usize,
) -> Result<Vp8x, DecodeError> {
    if payload.len() != 10 {
        return Err(error(
            DecodeErrorKind::InvalidContainer,
            payload_offset,
            "VP8X payload must be exactly 10 bytes",
        ));
    }
    let flags = Vp8xFlags(payload[0]);
    if profile == CompatibilityProfile::SpecStrict
        && (flags.reserved_bits() != 0 || payload[1..4].iter().any(|&byte| byte != 0))
    {
        return Err(error(
            DecodeErrorKind::InvalidContainer,
            payload_offset,
            "VP8X reserved fields are non-zero",
        ));
    }
    let width = read_u24(&payload[4..7]) + 1;
    let height = read_u24(&payload[7..10]) + 1;
    if width > limits.max_width || height > limits.max_height {
        return Err(error(
            DecodeErrorKind::LimitExceeded,
            payload_offset + 4,
            "canvas dimension exceeds limit",
        ));
    }
    let pixels = u64::from(width)
        .checked_mul(u64::from(height))
        .ok_or_else(|| {
            error(
                DecodeErrorKind::LimitExceeded,
                payload_offset + 4,
                "canvas pixel count overflow",
            )
        })?;
    if pixels > limits.max_pixels {
        return Err(error(
            DecodeErrorKind::LimitExceeded,
            payload_offset + 4,
            "canvas pixels exceed limit",
        ));
    }
    Ok(Vp8x {
        flags,
        canvas_width: width,
        canvas_height: height,
    })
}

fn validate_strict_layout(chunks: &[Chunk<'_>], vp8x: Option<Vp8x>) -> Result<(), DecodeError> {
    let mut lossy_count = 0u32;
    let mut lossless_count = 0u32;
    let mut alph_count = 0u32;
    let mut iccp_count = 0u32;
    let mut exif_count = 0u32;
    let mut xmp_count = 0u32;
    for chunk in chunks {
        match chunk.fourcc {
            VP8 => lossy_count += 1,
            VP8L => lossless_count += 1,
            ALPH => alph_count += 1,
            ICCP => iccp_count += 1,
            EXIF => exif_count += 1,
            XMP => xmp_count += 1,
            _ => {}
        }
    }
    if lossy_count > 1
        || lossless_count > 1
        || alph_count > 1
        || iccp_count > 1
        || exif_count > 1
        || xmp_count > 1
    {
        return Err(error(
            DecodeErrorKind::InvalidContainer,
            RIFF_HEADER_LEN,
            "duplicate singleton chunk",
        ));
    }
    if lossy_count > 0 && lossless_count > 0 {
        return Err(error(
            DecodeErrorKind::InvalidContainer,
            RIFF_HEADER_LEN,
            "both VP8 and VP8L chunks present",
        ));
    }
    if let Some(header) = vp8x {
        let first = chunks.first().expect("VP8X has a source chunk");
        if first.fourcc != VP8X {
            return Err(error(
                DecodeErrorKind::InvalidContainer,
                first.offset,
                "VP8X must be the first chunk",
            ));
        }
        let flags = header.flags;
        if flags.iccp() != (iccp_count == 1)
            || flags.exif() != (exif_count == 1)
            || flags.xmp() != (xmp_count == 1)
            // A VP8L payload can carry alpha itself; the container parser does
            // not inspect that bitstream.  Only an `ALPH` chunk is enough to
            // require the VP8X alpha feature bit at this layer.
            || (alph_count == 1 && !flags.alpha())
        {
            return Err(error(
                DecodeErrorKind::InvalidContainer,
                first.offset,
                "VP8X flags do not match present chunks",
            ));
        }
    } else if iccp_count != 0 || exif_count != 0 || xmp_count != 0 || alph_count != 0 {
        return Err(error(
            DecodeErrorKind::InvalidContainer,
            RIFF_HEADER_LEN,
            "extended chunks require VP8X",
        ));
    }
    Ok(())
}

fn first_payload<'a>(chunks: &[Chunk<'a>], fourcc: FourCc) -> Option<&'a [u8]> {
    chunks
        .iter()
        .find(|chunk| chunk.fourcc == fourcc)
        .map(|chunk| chunk.payload)
}

fn is_known(fourcc: FourCc) -> bool {
    matches!(
        fourcc,
        VP8 | VP8L | VP8X | ALPH | ICCP | EXIF | XMP | ANIM | ANMF
    )
}

fn read_u24(bytes: &[u8]) -> u32 {
    u32::from(bytes[0]) | (u32::from(bytes[1]) << 8) | (u32::from(bytes[2]) << 16)
}

fn read_u32(bytes: &[u8]) -> Result<u32, DecodeError> {
    let bytes: [u8; 4] = bytes.try_into().map_err(|_| {
        error(
            DecodeErrorKind::UnexpectedEof,
            0,
            "truncated little-endian u32",
        )
    })?;
    Ok(u32::from_le_bytes(bytes))
}

fn read_fourcc(bytes: &[u8]) -> Result<FourCc, DecodeError> {
    bytes
        .try_into()
        .map_err(|_| error(DecodeErrorKind::UnexpectedEof, 0, "truncated FourCC"))
}

fn error(kind: DecodeErrorKind, offset: usize, context: &'static str) -> DecodeError {
    DecodeError::at(kind, offset, context)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn limits() -> DecodeLimits {
        DecodeLimits::default()
    }

    fn riff(chunks: &[(FourCc, &[u8], Option<u8>)]) -> Vec<u8> {
        let mut body = b"WEBP".to_vec();
        for (fourcc, payload, padding) in chunks {
            body.extend_from_slice(fourcc);
            body.extend_from_slice(
                &u32::try_from(payload.len())
                    .expect("test payload must fit the RIFF u32 length")
                    .to_le_bytes(),
            );
            body.extend_from_slice(payload);
            if payload.len() % 2 == 1 {
                body.push(padding.unwrap_or(0));
            }
        }
        let mut output = b"RIFF".to_vec();
        output.extend_from_slice(
            &u32::try_from(body.len())
                .expect("test RIFF body must fit the u32 length")
                .to_le_bytes(),
        );
        output.extend_from_slice(&body);
        output
    }

    #[test]
    fn every_riff_prefix_is_an_error_except_the_complete_file() {
        let valid = riff(&[(VP8, &[1, 2], None)]);
        for prefix in 0..valid.len() {
            assert!(
                parse(
                    &valid[..prefix],
                    CompatibilityProfile::SpecStrict,
                    &limits()
                )
                .is_err(),
                "prefix {prefix}"
            );
        }
        assert!(parse(&valid, CompatibilityProfile::SpecStrict, &limits()).is_ok());
    }

    #[test]
    fn odd_padding_is_checked_by_profile() {
        let valid = riff(&[(VP8, &[9], Some(0))]);
        assert_eq!(
            parse(&valid, CompatibilityProfile::SpecStrict, &limits())
                .unwrap()
                .chunks()[0]
                .padding,
            Some(0)
        );
        let non_zero = riff(&[(VP8, &[9], Some(8))]);
        assert!(parse(&non_zero, CompatibilityProfile::SpecStrict, &limits()).is_err());
        assert!(
            parse(
                &non_zero,
                CompatibilityProfile::LibwebpCompatible,
                &limits()
            )
            .is_ok()
        );
    }

    #[test]
    fn compatible_profile_preserves_trailing_and_unknown_chunks() {
        let mut bytes = riff(&[(*b"zZZ!", &[7, 0, 8], Some(0)), (VP8, &[1, 2], None)]);
        bytes.extend_from_slice(&[0xaa, 0xbb]);
        assert!(parse(&bytes, CompatibilityProfile::SpecStrict, &limits()).is_err());
        let parsed = parse(&bytes, CompatibilityProfile::LibwebpCompatible, &limits()).unwrap();
        assert_eq!(parsed.trailing(), &[0xaa, 0xbb]);
        let unknown: Vec<_> = parsed.unknown_chunks().collect();
        assert_eq!(unknown.len(), 1);
        assert_eq!(unknown[0].fourcc, *b"zZZ!");
        assert_eq!(unknown[0].payload, &[7, 0, 8]);
    }

    #[test]
    fn truncated_large_chunk_size_does_not_overrun() {
        let mut bytes = b"RIFF".to_vec();
        bytes.extend_from_slice(&12u32.to_le_bytes());
        bytes.extend_from_slice(b"WEBPVP8 ");
        bytes.extend_from_slice(&u32::MAX.to_le_bytes());
        let error = parse(&bytes, CompatibilityProfile::SpecStrict, &limits()).unwrap_err();
        assert_eq!(error.kind(), DecodeErrorKind::UnexpectedEof);
    }

    #[test]
    fn vp8x_parses_canvas_and_extracts_raw_metadata() {
        let vp8x = [0b0010_1100, 0, 0, 0, 4, 0, 0, 2, 0, 0]; // 5x3, ICCP/EXIF/XMP
        let bytes = riff(&[
            (VP8X, &vp8x, None),
            (ICCP, &[1, 0], None),
            (EXIF, &[0xff], Some(0)),
            (XMP, b"x", Some(0)),
            (VP8, &[1, 2], None),
        ]);
        let parsed = parse(&bytes, CompatibilityProfile::SpecStrict, &limits()).unwrap();
        assert_eq!(parsed.vp8x().unwrap().canvas_width, 5);
        assert_eq!(parsed.vp8x().unwrap().canvas_height, 3);
        assert_eq!(
            parsed.metadata(),
            Metadata {
                iccp: Some(&[1, 0]),
                exif: Some(&[0xff]),
                xmp: Some(b"x")
            }
        );
    }

    #[test]
    fn strict_rejects_vp8x_metadata_flag_mismatch() {
        let vp8x = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let bytes = riff(&[
            (VP8X, &vp8x, None),
            (EXIF, &[1], Some(0)),
            (VP8, &[1, 2], None),
        ]);
        assert!(parse(&bytes, CompatibilityProfile::SpecStrict, &limits()).is_err());
        assert!(parse(&bytes, CompatibilityProfile::LibwebpCompatible, &limits()).is_ok());
    }

    #[test]
    fn reserved_vp8x_bits_are_a_profile_decision() {
        let vp8x = [0b1000_0000, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let bytes = riff(&[(VP8X, &vp8x, None), (VP8, &[1, 2], None)]);
        assert!(parse(&bytes, CompatibilityProfile::SpecStrict, &limits()).is_err());
        assert!(parse(&bytes, CompatibilityProfile::LibwebpCompatible, &limits()).is_ok());
    }
}
