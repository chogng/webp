//! Borrowed ANIM/ANMF models and their parsing invariants.

use crate::ALPH;
use crate::ANIM;
use crate::ANMF;
use crate::Chunk;
use crate::CompatibilityProfile;
use crate::ContainerError;
use crate::ContainerErrorKind;
use crate::ContainerLimits;
use crate::VP8;
use crate::VP8L;
use crate::Vp8x;
use crate::arithmetic::checked_chunk_end;
use crate::arithmetic::checked_rect_end;
use webp_utils::read_u24_le;

const CHUNK_HEADER_LEN: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Animation<'a> {
    pub background_bgra: [u8; 4],
    pub loop_count: u16,
    pub(crate) frames: Vec<AnimationFrame<'a>>,
}

impl<'a> Animation<'a> {
    /// Returns the number of validated ANMF frames.
    #[must_use]
    pub fn frame_count(&self) -> usize {
        self.frames.len()
    }

    /// Returns a frame by its display-order index.
    #[must_use]
    pub fn frame(&self, index: usize) -> Option<&AnimationFrame<'a>> {
        self.frames.get(index)
    }

    /// Returns all validated frames in display order.
    #[must_use]
    pub fn frames(&self) -> &[AnimationFrame<'a>] {
        &self.frames
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnimationFrame<'a> {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub duration_ms: u32,
    pub dispose_to_background: bool,
    pub blend: bool,
    pub alpha: Option<&'a [u8]>,
    pub bitstream: FrameBitstream<'a>,
}

impl AnimationFrame<'_> {
    /// Returns whether ALPH or the VP8L fixed header signals transparency.
    ///
    /// The VP8L bit is a coding hint; actual pixel alpha remains a decoder
    /// responsibility.
    #[must_use]
    pub fn has_alpha_hint(&self) -> bool {
        self.alpha.is_some()
            || match self.bitstream {
                FrameBitstream::Vp8(_) => false,
                FrameBitstream::Vp8l(payload) => payload
                    .get(1..5)
                    .and_then(|bytes| bytes.try_into().ok())
                    .map(u32::from_le_bytes)
                    .is_some_and(|fields| fields & (1 << 28) != 0),
            }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameBitstream<'a> {
    Vp8(&'a [u8]),
    Vp8l(&'a [u8]),
}

/// Parses validated ANIM and ANMF state after top-level RIFF framing.
pub(crate) fn parse_animation<'a>(
    chunks: &[Chunk<'a>],
    vp8x: Option<Vp8x>,
    profile: CompatibilityProfile,
    limits: &ContainerLimits,
) -> Result<Option<Animation<'a>>, ContainerError> {
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
                ContainerErrorKind::InvalidContainer,
                12,
                "missing ANIM chunk",
            )
        })?;
    if anim.payload.len() != 6 {
        return Err(error(
            ContainerErrorKind::InvalidContainer,
            anim.offset + CHUNK_HEADER_LEN,
            "ANIM payload must be exactly 6 bytes",
        ));
    }
    let background_bgra: [u8; 4] = anim.payload[..4].try_into().expect("fixed ANIM color");
    let loop_count = u16::from_le_bytes([anim.payload[4], anim.payload[5]]);
    let mut total_pixels = 0_u64;
    let mut total_frame_chunks = 0_u32;
    let mut frames = Vec::new();
    for chunk in chunks.iter().filter(|chunk| chunk.fourcc == ANMF) {
        if frames.len() >= limits.max_frames as usize {
            return Err(error(
                ContainerErrorKind::LimitExceeded,
                chunk.offset,
                "animation exceeds max_frames",
            ));
        }
        let frame = parse_anmf(chunk, vp8x, profile, limits, &mut total_frame_chunks)?;
        let frame_pixels = u64::from(frame.width)
            .checked_mul(u64::from(frame.height))
            .ok_or_else(|| {
                error(
                    ContainerErrorKind::LimitExceeded,
                    chunk.offset,
                    "animation pixel count overflow",
                )
            })?;
        total_pixels = total_pixels.checked_add(frame_pixels).ok_or_else(|| {
            error(
                ContainerErrorKind::LimitExceeded,
                chunk.offset,
                "animation pixel count overflow",
            )
        })?;
        if total_pixels > limits.max_total_frame_pixels {
            return Err(error(
                ContainerErrorKind::LimitExceeded,
                chunk.offset,
                "animation pixels exceed max_total_frame_pixels",
            ));
        }
        frames.try_reserve(1).map_err(|_| {
            error(
                ContainerErrorKind::AllocationFailed,
                chunk.offset,
                "animation frame storage allocation failed",
            )
        })?;
        frames.push(frame);
    }
    if frames.is_empty() {
        return Err(error(
            ContainerErrorKind::InvalidContainer,
            anim.offset,
            "animated WebP has no ANMF frames",
        ));
    }
    if profile == CompatibilityProfile::SpecStrict
        && frames.iter().any(AnimationFrame::has_alpha_hint)
        && !vp8x.flags.alpha()
    {
        return Err(error(
            ContainerErrorKind::InvalidContainer,
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
    limits: &ContainerLimits,
    total_frame_chunks: &mut u32,
) -> Result<AnimationFrame<'a>, ContainerError> {
    const ANMF_HEADER_LEN: usize = 16;
    if chunk.payload.len() < ANMF_HEADER_LEN {
        return Err(error(
            ContainerErrorKind::UnexpectedEof,
            chunk.offset + CHUNK_HEADER_LEN + chunk.payload.len(),
            "truncated ANMF header",
        ));
    }
    let x = read_u24_le(chunk.payload[..3].try_into().expect("validated ANMF x"))
        .checked_mul(2)
        .ok_or_else(|| {
            error(
                ContainerErrorKind::InvalidContainer,
                chunk.offset + CHUNK_HEADER_LEN,
                "ANMF x overflow",
            )
        })?;
    let y = read_u24_le(chunk.payload[3..6].try_into().expect("validated ANMF y"))
        .checked_mul(2)
        .ok_or_else(|| {
            error(
                ContainerErrorKind::InvalidContainer,
                chunk.offset + CHUNK_HEADER_LEN + 3,
                "ANMF y overflow",
            )
        })?;
    let width = read_u24_le(
        chunk.payload[6..9]
            .try_into()
            .expect("validated ANMF width"),
    ) + 1;
    let height = read_u24_le(
        chunk.payload[9..12]
            .try_into()
            .expect("validated ANMF height"),
    ) + 1;
    checked_rect_end(x, width, vp8x.canvas_width).map_err(|_| {
        error(
            ContainerErrorKind::InvalidContainer,
            chunk.offset + CHUNK_HEADER_LEN,
            "ANMF frame exceeds canvas width",
        )
    })?;
    checked_rect_end(y, height, vp8x.canvas_height).map_err(|_| {
        error(
            ContainerErrorKind::InvalidContainer,
            chunk.offset + CHUNK_HEADER_LEN + 3,
            "ANMF frame exceeds canvas height",
        )
    })?;
    let duration_ms = read_u24_le(
        chunk.payload[12..15]
            .try_into()
            .expect("validated ANMF duration"),
    );
    let flags = chunk.payload[15];
    // Reserved bits are ignored by readers for forward compatibility.

    let nested = &chunk.payload[ANMF_HEADER_LEN..];
    let mut offset = 0_usize;
    let mut alpha = None;
    let mut bitstream = None;
    let mut image_header = None;
    let mut seen_unknown = false;
    while offset < nested.len() {
        if nested.len() - offset < CHUNK_HEADER_LEN {
            return Err(error(
                ContainerErrorKind::UnexpectedEof,
                chunk.offset + CHUNK_HEADER_LEN + ANMF_HEADER_LEN + offset,
                "truncated ANMF subchunk header",
            ));
        }
        let fourcc = nested[offset..offset + 4]
            .try_into()
            .expect("checked ANMF FourCC");
        let size = u32::from_le_bytes(
            nested[offset + 4..offset + 8]
                .try_into()
                .expect("checked ANMF size"),
        );
        let next = checked_chunk_end(offset, size, nested.len())?;
        if *total_frame_chunks >= limits.max_chunks {
            return Err(error(
                ContainerErrorKind::LimitExceeded,
                chunk.offset + CHUNK_HEADER_LEN + ANMF_HEADER_LEN + offset,
                "animation nested chunks exceed max_chunks",
            ));
        }
        *total_frame_chunks += 1;
        let padding_len = (size & 1) as usize;
        let payload_end = next - padding_len;
        if profile == CompatibilityProfile::SpecStrict
            && padding_len == 1
            && nested[payload_end] != 0
        {
            return Err(error(
                ContainerErrorKind::InvalidContainer,
                chunk.offset + CHUNK_HEADER_LEN + ANMF_HEADER_LEN + payload_end,
                "non-zero ANMF subchunk padding",
            ));
        }
        let payload = &nested[offset + CHUNK_HEADER_LEN..payload_end];
        match fourcc {
            ALPH => {
                if (profile == CompatibilityProfile::SpecStrict && seen_unknown)
                    || alpha.replace(payload).is_some()
                    || bitstream.is_some()
                {
                    return Err(error(
                        ContainerErrorKind::InvalidContainer,
                        chunk.offset + CHUNK_HEADER_LEN + ANMF_HEADER_LEN + offset,
                        "ANMF ALPH must appear once before the bitstream",
                    ));
                }
            }
            VP8 => {
                if (profile == CompatibilityProfile::SpecStrict && seen_unknown)
                    || bitstream.is_some()
                {
                    return Err(error(
                        ContainerErrorKind::InvalidContainer,
                        chunk.offset + CHUNK_HEADER_LEN + ANMF_HEADER_LEN + offset,
                        "ANMF contains multiple image bitstreams",
                    ));
                }
                image_header = Some(crate::image_header::parse(
                    VP8,
                    payload,
                    limits,
                    chunk.offset + CHUNK_HEADER_LEN + ANMF_HEADER_LEN + offset + CHUNK_HEADER_LEN,
                )?);
                bitstream = Some(FrameBitstream::Vp8(payload));
            }
            VP8L => {
                if (profile == CompatibilityProfile::SpecStrict && seen_unknown)
                    || alpha.is_some()
                    || bitstream.is_some()
                {
                    return Err(error(
                        ContainerErrorKind::InvalidContainer,
                        chunk.offset + CHUNK_HEADER_LEN + ANMF_HEADER_LEN + offset,
                        "ANMF ALPH cannot accompany VP8L or duplicate a bitstream",
                    ));
                }
                image_header = Some(crate::image_header::parse(
                    VP8L,
                    payload,
                    limits,
                    chunk.offset + CHUNK_HEADER_LEN + ANMF_HEADER_LEN + offset + CHUNK_HEADER_LEN,
                )?);
                bitstream = Some(FrameBitstream::Vp8l(payload));
            }
            _ if profile == CompatibilityProfile::SpecStrict
                && webp_container::is_known(fourcc) =>
            {
                return Err(error(
                    ContainerErrorKind::InvalidContainer,
                    chunk.offset + CHUNK_HEADER_LEN + ANMF_HEADER_LEN + offset,
                    "known top-level chunk is not valid ANMF frame data",
                ));
            }
            _ if profile == CompatibilityProfile::SpecStrict => seen_unknown = true,
            _ => {}
        }
        offset = next;
    }
    let bitstream = bitstream.ok_or_else(|| {
        error(
            ContainerErrorKind::InvalidContainer,
            chunk.offset + CHUNK_HEADER_LEN + ANMF_HEADER_LEN,
            "ANMF frame has no VP8 or VP8L bitstream",
        )
    })?;
    let image_header = image_header.expect("bitstream and fixed header are stored together");
    if image_header.width != width || image_header.height != height {
        return Err(error(
            ContainerErrorKind::InvalidDimensions,
            chunk.offset + CHUNK_HEADER_LEN + ANMF_HEADER_LEN,
            "ANMF dimensions do not match its image bitstream",
        ));
    }
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

fn error(kind: ContainerErrorKind, offset: usize, context: &'static str) -> ContainerError {
    ContainerError::at(kind, offset, context)
}

#[cfg(test)]
#[path = "animation_tests.rs"]
mod tests;
