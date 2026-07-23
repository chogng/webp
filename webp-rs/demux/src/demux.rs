//! Safe, zero-copy parsing of the WebP RIFF container.
//!
//! This crate deliberately stops at the container boundary. It validates RIFF
//! lengths and chunk framing, exposes unknown chunks unchanged, and decodes the
//! small `VP8X` header without attempting to decode VP8 or VP8L payloads.

use crate::Chunk;
use crate::CompatibilityProfile;
use crate::ContainerError;
use crate::ContainerErrorKind;
use crate::ContainerLimits;
use crate::DemuxOptions;
use crate::EXIF;
use crate::FourCc;
use crate::ICCP;
use crate::Metadata;
use crate::VP8;
use crate::VP8L;
use crate::VP8X;
use crate::Vp8x;
use crate::Vp8xFlags;
use crate::XMP;
use crate::arithmetic::checked_chunk_end;
use webp_container::is_known;
use webp_utils::read_u24_le;

const RIFF_HEADER_LEN: usize = 12;
const CHUNK_HEADER_LEN: usize = 8;

/// A complete zero-copy WebP container view.
///
/// `Demuxer` validates RIFF framing and the selected layout profile, but keeps
/// VP8, VP8L, and ALPH payloads opaque. All returned byte slices borrow from
/// the input passed to [`Demuxer::parse`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Demuxer<'a> {
    chunks: Vec<Chunk<'a>>,
    vp8x: Option<Vp8x>,
    image: Option<StillImage<'a>>,
    animation: Option<crate::Animation<'a>>,
    /// Bytes outside the declared RIFF length.  This is only populated in the
    /// compatible profile; strict parsing rejects such input.
    trailing: &'a [u8],
}

impl<'a> Demuxer<'a> {
    /// Parses one complete WebP RIFF container using a reusable demux policy.
    ///
    /// # Errors
    ///
    /// Returns a [`ContainerError`] when RIFF framing, resource limits, or
    /// the selected layout profile rejects the input.
    pub fn parse(data: &'a [u8], options: &DemuxOptions) -> Result<Self, ContainerError> {
        parse_with(data, options.profile, &options.limits)
    }

    /// Returns the number of retained top-level RIFF chunks.
    #[must_use]
    pub fn chunk_count(&self) -> usize {
        self.chunks.len()
    }

    /// Returns one top-level RIFF chunk by index.
    #[must_use]
    pub fn chunk(&self, index: usize) -> Option<&Chunk<'a>> {
        self.chunks.get(index)
    }

    /// Iterates every top-level chunk with the requested FourCC in wire order.
    pub fn chunks_with(&self, fourcc: FourCc) -> impl Iterator<Item = &Chunk<'a>> {
        self.chunks
            .iter()
            .filter(move |chunk| chunk.fourcc == fourcc)
    }

    /// Returns the validated opaque static-image payload, when present.
    ///
    /// Compatibility parsing can retain non-standard layouts. For those
    /// inputs this returns the first top-level VP8 or VP8L chunk in wire order;
    /// callers which need every raw occurrence can use [`Demuxer::chunks_with`].
    #[must_use]
    pub fn image(&self) -> Option<StillImage<'a>> {
        self.image
    }

    #[must_use]
    pub fn chunks(&self) -> &[Chunk<'a>] {
        &self.chunks
    }

    #[must_use]
    pub fn vp8x(&self) -> Option<Vp8x> {
        self.vp8x
    }

    /// Returns the canvas dimensions for either simple or extended WebP.
    #[must_use]
    pub fn canvas_dimensions(&self) -> Option<(u32, u32)> {
        self.vp8x
            .map(|header| (header.canvas_width, header.canvas_height))
            .or_else(|| self.image.map(|image| (image.width, image.height)))
    }

    /// Returns whether the parsed image is animated.
    #[must_use]
    pub fn is_animated(&self) -> bool {
        self.animation.is_some()
    }

    /// Returns the number of image frames exposed by the parsed container.
    #[must_use]
    pub fn frame_count(&self) -> usize {
        self.animation.as_ref().map_or(
            usize::from(self.image.is_some()),
            crate::Animation::frame_count,
        )
    }

    /// Animation control data and validated frame descriptors, when present.
    #[must_use]
    pub fn animation(&self) -> Option<&crate::Animation<'a>> {
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

/// Backward-compatible name for a parsed container.
///
/// New APIs should use [`Demuxer`].
pub type Container<'a> = Demuxer<'a>;

/// One opaque static image payload selected from the top-level chunk layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StillImage<'a> {
    alpha: Option<&'a [u8]>,
    bitstream: ImageBitstream<'a>,
    width: u32,
    height: u32,
    alpha_hint: bool,
}

impl<'a> StillImage<'a> {
    /// Returns the raw top-level ALPH payload, when the container provides one.
    #[must_use]
    pub fn alpha(self) -> Option<&'a [u8]> {
        self.alpha
    }

    /// Returns the opaque VP8 or VP8L payload.
    #[must_use]
    pub fn bitstream(self) -> ImageBitstream<'a> {
        self.bitstream
    }

    /// Returns the width declared by the fixed VP8 or VP8L header.
    #[must_use]
    pub fn width(self) -> u32 {
        self.width
    }

    /// Returns the height declared by the fixed VP8 or VP8L header.
    #[must_use]
    pub fn height(self) -> u32 {
        self.height
    }

    /// Returns whether the container or lossless header signals alpha.
    ///
    /// The VP8L bit is a coding hint; actual pixel alpha remains a decoder
    /// responsibility.
    #[must_use]
    pub fn has_alpha_hint(self) -> bool {
        self.alpha.is_some() || self.alpha_hint
    }
}

/// Codec kind and opaque payload for a static WebP image.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageBitstream<'a> {
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
pub fn parse<'a>(
    data: &'a [u8],
    profile: CompatibilityProfile,
    limits: &ContainerLimits,
) -> Result<Demuxer<'a>, ContainerError> {
    parse_with(data, profile, limits)
}

#[allow(clippy::too_many_lines)] // Keep the linear parser's boundary checks adjacent.
fn parse_with<'a>(
    data: &'a [u8],
    profile: CompatibilityProfile,
    limits: &ContainerLimits,
) -> Result<Demuxer<'a>, ContainerError> {
    if data.len() > limits.max_input_bytes {
        return Err(error(
            ContainerErrorKind::LimitExceeded,
            0,
            "input exceeds max_input_bytes",
        ));
    }
    if data.len() < RIFF_HEADER_LEN {
        return Err(error(
            ContainerErrorKind::UnexpectedEof,
            data.len(),
            "truncated RIFF header",
        ));
    }
    if data[..4] != *b"RIFF" || data[8..12] != *b"WEBP" {
        return Err(error(
            ContainerErrorKind::InvalidContainer,
            0,
            "missing RIFF/WEBP magic",
        ));
    }

    let declared = read_u32(&data[4..8])?;
    if declared < 4 {
        return Err(error(
            ContainerErrorKind::InvalidContainer,
            4,
            "RIFF size excludes WEBP form type",
        ));
    }
    let container_end = 8usize.checked_add(declared as usize).ok_or_else(|| {
        error(
            ContainerErrorKind::InvalidContainer,
            4,
            "RIFF size overflow",
        )
    })?;
    if container_end > data.len() {
        return Err(error(
            ContainerErrorKind::UnexpectedEof,
            data.len(),
            "RIFF body is truncated",
        ));
    }
    if profile == CompatibilityProfile::SpecStrict && container_end != data.len() {
        return Err(error(
            ContainerErrorKind::InvalidContainer,
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
                ContainerErrorKind::UnexpectedEof,
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
                ContainerErrorKind::InvalidContainer,
                payload_end,
                "non-zero RIFF padding",
            ));
        }

        if matches!(fourcc, ICCP | EXIF | XMP) {
            metadata_bytes = metadata_bytes.checked_add(payload.len()).ok_or_else(|| {
                error(
                    ContainerErrorKind::LimitExceeded,
                    offset,
                    "metadata size overflow",
                )
            })?;
            if metadata_bytes > limits.max_metadata_bytes {
                return Err(error(
                    ContainerErrorKind::LimitExceeded,
                    offset,
                    "metadata exceeds max_metadata_bytes",
                ));
            }
        }

        if fourcc == VP8X {
            if vp8x.is_some() && profile == CompatibilityProfile::SpecStrict {
                return Err(error(
                    ContainerErrorKind::InvalidContainer,
                    offset,
                    "duplicate VP8X chunk",
                ));
            }
            let parsed = parse_vp8x(payload, limits, offset + CHUNK_HEADER_LEN)?;
            if vp8x.is_none() {
                vp8x = Some(parsed);
            }
        }
        if chunks.len() >= limits.max_chunks as usize {
            return Err(error(
                ContainerErrorKind::LimitExceeded,
                offset,
                "container exceeds max_chunks",
            ));
        }
        chunks.try_reserve(1).map_err(|_| {
            error(
                ContainerErrorKind::AllocationFailed,
                offset,
                "chunk storage allocation failed",
            )
        })?;
        chunks.push(Chunk {
            fourcc,
            payload,
            padding,
            offset,
        });
        offset = next;
    }

    if profile == CompatibilityProfile::SpecStrict {
        crate::layout::validate_strict_layout(&chunks, vp8x)?;
    }
    let image = still_image(&chunks, vp8x, limits)?;
    if profile == CompatibilityProfile::SpecStrict
        && image.is_some_and(StillImage::has_alpha_hint)
        && vp8x.is_some_and(|header| !header.flags.alpha())
    {
        return Err(error(
            ContainerErrorKind::InvalidContainer,
            RIFF_HEADER_LEN,
            "VP8X alpha flag is missing for the static image",
        ));
    }
    let animation = crate::animation::parse_animation(&chunks, vp8x, profile, limits)?;
    Ok(Demuxer {
        chunks,
        vp8x,
        image,
        animation,
        trailing: &data[container_end..],
    })
}

fn parse_vp8x(
    payload: &[u8],
    limits: &ContainerLimits,
    payload_offset: usize,
) -> Result<Vp8x, ContainerError> {
    if payload.len() != 10 {
        return Err(error(
            ContainerErrorKind::InvalidContainer,
            payload_offset,
            "VP8X payload must be exactly 10 bytes",
        ));
    }
    let flags = Vp8xFlags::from_bits(payload[0]);
    // Reserved fields are writer-zero fields, but the WebP reader contract
    // requires ignoring them for forward compatibility in every profile.
    let width = read_u24_le(payload[4..7].try_into().expect("validated VP8X width")) + 1;
    let height = read_u24_le(payload[7..10].try_into().expect("validated VP8X height")) + 1;
    if width > limits.max_width || height > limits.max_height {
        return Err(error(
            ContainerErrorKind::LimitExceeded,
            payload_offset + 4,
            "canvas dimension exceeds limit",
        ));
    }
    let pixels = u64::from(width)
        .checked_mul(u64::from(height))
        .ok_or_else(|| {
            error(
                ContainerErrorKind::LimitExceeded,
                payload_offset + 4,
                "canvas pixel count overflow",
            )
        })?;
    if pixels > u64::from(u32::MAX) {
        return Err(error(
            ContainerErrorKind::InvalidDimensions,
            payload_offset + 4,
            "VP8X canvas exceeds the WebP pixel-product limit",
        ));
    }
    if pixels > limits.max_pixels {
        return Err(error(
            ContainerErrorKind::LimitExceeded,
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

fn still_image<'a>(
    chunks: &[Chunk<'a>],
    vp8x: Option<Vp8x>,
    limits: &ContainerLimits,
) -> Result<Option<StillImage<'a>>, ContainerError> {
    let Some(chunk) = chunks
        .iter()
        .find(|chunk| matches!(chunk.fourcc, VP8 | VP8L))
    else {
        return Ok(None);
    };
    let header = crate::image_header::parse(
        chunk.fourcc,
        chunk.payload,
        limits,
        chunk.offset + CHUNK_HEADER_LEN,
    )?;
    if let Some(canvas) = vp8x
        && (header.width != canvas.canvas_width || header.height != canvas.canvas_height)
    {
        return Err(error(
            ContainerErrorKind::InvalidDimensions,
            chunk.offset + CHUNK_HEADER_LEN,
            "VP8X canvas does not match static image dimensions",
        ));
    }
    let bitstream = match chunk.fourcc {
        VP8 => ImageBitstream::Vp8(chunk.payload),
        VP8L => ImageBitstream::Vp8l(chunk.payload),
        _ => unreachable!("selected only VP8 or VP8L"),
    };
    Ok(Some(StillImage {
        alpha: first_payload(chunks, crate::ALPH),
        bitstream,
        width: header.width,
        height: header.height,
        alpha_hint: header.alpha_hint,
    }))
}

fn first_payload<'a>(chunks: &[Chunk<'a>], fourcc: FourCc) -> Option<&'a [u8]> {
    chunks
        .iter()
        .find(|chunk| chunk.fourcc == fourcc)
        .map(|chunk| chunk.payload)
}

fn read_u32(bytes: &[u8]) -> Result<u32, ContainerError> {
    let bytes: [u8; 4] = bytes.try_into().map_err(|_| {
        error(
            ContainerErrorKind::UnexpectedEof,
            0,
            "truncated little-endian u32",
        )
    })?;
    Ok(u32::from_le_bytes(bytes))
}

fn read_fourcc(bytes: &[u8]) -> Result<FourCc, ContainerError> {
    bytes
        .try_into()
        .map_err(|_| error(ContainerErrorKind::UnexpectedEof, 0, "truncated FourCC"))
}

fn error(kind: ContainerErrorKind, offset: usize, context: &'static str) -> ContainerError {
    ContainerError::at(kind, offset, context)
}

#[cfg(test)]
#[path = "demux_tests.rs"]
mod container_tests;
