//! Safe, zero-copy parsing of the WebP RIFF container.
//!
//! This crate deliberately stops at the container boundary.  It validates RIFF
//! lengths and chunk framing, exposes unknown chunks unchanged, and decodes the
//! small `VP8X` header without attempting to decode VP8 or VP8L payloads.

use webp_core::CompatibilityProfile;
use webp_core::DecodeError;
use webp_core::DecodeErrorKind;
use webp_core::DecodeLimits;
use webp_core::checked_chunk_end;
use webp_core::checked_rect_end;

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
    animation: Option<Animation<'a>>,
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

    /// Animation control data and validated frame descriptors, when present.
    #[must_use]
    pub fn animation(&self) -> Option<&Animation<'a>> {
        self.animation.as_ref()
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

/// Parsed global animation parameters and its display-order frames.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Animation<'a> {
    /// The default canvas color in the WebP wire order: BGRA.
    pub background_bgra: [u8; 4],
    /// `0` represents infinitely many loops.
    pub loop_count: u16,
    frames: Vec<AnimationFrame<'a>>,
}

impl<'a> Animation<'a> {
    #[must_use]
    pub fn frames(&self) -> &[AnimationFrame<'a>] {
        &self.frames
    }
}

/// A validated `ANMF` frame descriptor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnimationFrame<'a> {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    /// The bitstream value in milliseconds, preserved without player policy.
    pub duration_ms: u32,
    pub dispose_to_background: bool,
    /// `true` means alpha-blend; `false` means overwrite the frame rectangle.
    pub blend: bool,
    pub alpha: Option<&'a [u8]>,
    pub bitstream: FrameBitstream<'a>,
}

/// The frame-local WebP image bitstream carried by an `ANMF` chunk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameBitstream<'a> {
    Vp8(&'a [u8]),
    Vp8l(&'a [u8]),
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
    let animation = parse_animation(&chunks, vp8x, profile, limits)?;
    Ok(Container {
        chunks,
        vp8x,
        animation,
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
    let mut anim_count = 0u32;
    let mut anmf_count = 0u32;
    for chunk in chunks {
        match chunk.fourcc {
            VP8 => lossy_count += 1,
            VP8L => lossless_count += 1,
            ALPH => alph_count += 1,
            ICCP => iccp_count += 1,
            EXIF => exif_count += 1,
            XMP => xmp_count += 1,
            ANIM => anim_count += 1,
            ANMF => anmf_count += 1,
            _ => {}
        }
    }
    if lossy_count > 1
        || lossless_count > 1
        || alph_count > 1
        || iccp_count > 1
        || exif_count > 1
        || xmp_count > 1
        || anim_count > 1
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
    if anmf_count > 0 && (lossy_count > 0 || lossless_count > 0 || alph_count > 0) {
        return Err(error(
            DecodeErrorKind::InvalidContainer,
            RIFF_HEADER_LEN,
            "animated and still-image chunks cannot be mixed",
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
            || flags.animation() != (anim_count == 1 && anmf_count != 0)
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
    } else if iccp_count != 0
        || exif_count != 0
        || xmp_count != 0
        || alph_count != 0
        || anim_count != 0
        || anmf_count != 0
    {
        return Err(error(
            DecodeErrorKind::InvalidContainer,
            RIFF_HEADER_LEN,
            "extended chunks require VP8X",
        ));
    }
    Ok(())
}

fn parse_animation<'a>(
    chunks: &[Chunk<'a>],
    vp8x: Option<Vp8x>,
    profile: CompatibilityProfile,
    limits: &DecodeLimits,
) -> Result<Option<Animation<'a>>, DecodeError> {
    let Some(vp8x) = vp8x else {
        return Ok(None);
    };
    if !vp8x.flags.animation() {
        return Ok(None);
    }
    let anim = chunks
        .iter()
        .find(|chunk| chunk.fourcc == ANIM)
        .ok_or_else(|| {
            error(
                DecodeErrorKind::InvalidContainer,
                RIFF_HEADER_LEN,
                "missing ANIM chunk",
            )
        })?;
    if anim.payload.len() != 6 {
        return Err(error(
            DecodeErrorKind::InvalidContainer,
            anim.offset + CHUNK_HEADER_LEN,
            "ANIM payload must be exactly 6 bytes",
        ));
    }
    let background_bgra: [u8; 4] = anim.payload[..4].try_into().expect("fixed ANIM color");
    let loop_count = u16::from_le_bytes([anim.payload[4], anim.payload[5]]);
    let mut total_pixels = 0_u64;
    let mut frames = Vec::new();
    for chunk in chunks.iter().filter(|chunk| chunk.fourcc == ANMF) {
        if frames.len() >= limits.max_frames as usize {
            return Err(error(
                DecodeErrorKind::LimitExceeded,
                chunk.offset,
                "animation exceeds max_frames",
            ));
        }
        let frame = parse_anmf(chunk, vp8x, profile, limits)?;
        total_pixels = total_pixels
            .checked_add(u64::from(frame.width) * u64::from(frame.height))
            .ok_or_else(|| {
                error(
                    DecodeErrorKind::LimitExceeded,
                    chunk.offset,
                    "animation pixel count overflow",
                )
            })?;
        if total_pixels > limits.max_total_frame_pixels {
            return Err(error(
                DecodeErrorKind::LimitExceeded,
                chunk.offset,
                "animation pixels exceed max_total_frame_pixels",
            ));
        }
        frames.push(frame);
    }
    if frames.is_empty() {
        return Err(error(
            DecodeErrorKind::InvalidContainer,
            anim.offset,
            "animated WebP has no ANMF frames",
        ));
    }
    if profile == CompatibilityProfile::SpecStrict
        && frames.iter().any(|frame| frame.alpha.is_some())
        && !vp8x.flags.alpha()
    {
        return Err(error(
            DecodeErrorKind::InvalidContainer,
            anim.offset,
            "VP8X alpha flag is missing for an ANMF ALPH chunk",
        ));
    }
    Ok(Some(Animation {
        background_bgra,
        loop_count,
        frames,
    }))
}

fn parse_anmf<'a>(
    chunk: &Chunk<'a>,
    vp8x: Vp8x,
    profile: CompatibilityProfile,
    _limits: &DecodeLimits,
) -> Result<AnimationFrame<'a>, DecodeError> {
    const ANMF_HEADER_LEN: usize = 16;
    if chunk.payload.len() < ANMF_HEADER_LEN {
        return Err(error(
            DecodeErrorKind::UnexpectedEof,
            chunk.offset + CHUNK_HEADER_LEN + chunk.payload.len(),
            "truncated ANMF header",
        ));
    }
    let x = read_u24(&chunk.payload[..3])
        .checked_mul(2)
        .ok_or_else(|| {
            error(
                DecodeErrorKind::InvalidContainer,
                chunk.offset + CHUNK_HEADER_LEN,
                "ANMF x overflow",
            )
        })?;
    let y = read_u24(&chunk.payload[3..6])
        .checked_mul(2)
        .ok_or_else(|| {
            error(
                DecodeErrorKind::InvalidContainer,
                chunk.offset + CHUNK_HEADER_LEN + 3,
                "ANMF y overflow",
            )
        })?;
    let width = read_u24(&chunk.payload[6..9]) + 1;
    let height = read_u24(&chunk.payload[9..12]) + 1;
    checked_rect_end(x, width, vp8x.canvas_width).map_err(|_| {
        error(
            DecodeErrorKind::InvalidContainer,
            chunk.offset + CHUNK_HEADER_LEN,
            "ANMF frame exceeds canvas width",
        )
    })?;
    checked_rect_end(y, height, vp8x.canvas_height).map_err(|_| {
        error(
            DecodeErrorKind::InvalidContainer,
            chunk.offset + CHUNK_HEADER_LEN + 3,
            "ANMF frame exceeds canvas height",
        )
    })?;
    let duration_ms = read_u24(&chunk.payload[12..15]);
    let flags = chunk.payload[15];
    if profile == CompatibilityProfile::SpecStrict && flags & !0b11 != 0 {
        return Err(error(
            DecodeErrorKind::InvalidContainer,
            chunk.offset + CHUNK_HEADER_LEN + 15,
            "ANMF reserved bits are non-zero",
        ));
    }

    let nested = &chunk.payload[ANMF_HEADER_LEN..];
    let mut offset = 0_usize;
    let mut alpha = None;
    let mut bitstream = None;
    while offset < nested.len() {
        if nested.len() - offset < CHUNK_HEADER_LEN {
            return Err(error(
                DecodeErrorKind::UnexpectedEof,
                chunk.offset + CHUNK_HEADER_LEN + ANMF_HEADER_LEN + offset,
                "truncated ANMF subchunk header",
            ));
        }
        let fourcc = read_fourcc(&nested[offset..offset + 4])?;
        let size = read_u32(&nested[offset + 4..offset + 8])?;
        let next = checked_chunk_end(offset, size, nested.len())?;
        let padding_len = (size & 1) as usize;
        let payload_end = next - padding_len;
        if profile == CompatibilityProfile::SpecStrict
            && padding_len == 1
            && nested[payload_end] != 0
        {
            return Err(error(
                DecodeErrorKind::InvalidContainer,
                chunk.offset + CHUNK_HEADER_LEN + ANMF_HEADER_LEN + payload_end,
                "non-zero ANMF subchunk padding",
            ));
        }
        let payload = &nested[offset + CHUNK_HEADER_LEN..payload_end];
        match fourcc {
            ALPH => {
                if alpha.replace(payload).is_some() || bitstream.is_some() {
                    return Err(error(
                        DecodeErrorKind::InvalidContainer,
                        chunk.offset + CHUNK_HEADER_LEN + ANMF_HEADER_LEN + offset,
                        "ANMF ALPH must appear once before the bitstream",
                    ));
                }
            }
            VP8 => {
                if bitstream.replace(FrameBitstream::Vp8(payload)).is_some() {
                    return Err(error(
                        DecodeErrorKind::InvalidContainer,
                        chunk.offset + CHUNK_HEADER_LEN + ANMF_HEADER_LEN + offset,
                        "ANMF contains multiple image bitstreams",
                    ));
                }
            }
            VP8L if alpha.is_some()
                || bitstream.replace(FrameBitstream::Vp8l(payload)).is_some() =>
            {
                return Err(error(
                    DecodeErrorKind::InvalidContainer,
                    chunk.offset + CHUNK_HEADER_LEN + ANMF_HEADER_LEN + offset,
                    "ANMF ALPH cannot accompany VP8L or duplicate a bitstream",
                ));
            }
            VP8L => {}
            _ => {}
        }
        offset = next;
    }
    let bitstream = bitstream.ok_or_else(|| {
        error(
            DecodeErrorKind::InvalidContainer,
            chunk.offset + CHUNK_HEADER_LEN + ANMF_HEADER_LEN,
            "ANMF frame has no VP8 or VP8L bitstream",
        )
    })?;
    Ok(AnimationFrame {
        x,
        y,
        width,
        height,
        duration_ms,
        dispose_to_background: flags & 1 != 0,
        blend: flags & 2 == 0,
        alpha,
        bitstream,
    })
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
#[path = "container_tests.rs"]
mod container_tests;

#[cfg(test)]
#[path = "animation_tests.rs"]
mod animation_tests;
