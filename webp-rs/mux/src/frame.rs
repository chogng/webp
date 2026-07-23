//! Animation-frame input models and ANMF payload serialization.

use crate::ALPH;
use crate::ContainerError;
use crate::ContainerErrorKind;
use crate::VP8;
use crate::VP8L;
use crate::wire::chunk_storage_len;
use crate::wire::dimensions_fit_u24_minus_one;
use crate::wire::error;
use crate::wire::push_chunk;
use crate::wire::reserve;
use crate::wire::size_overflow;

const MAX_ANIMATION_DURATION_MS: u32 = (1 << 24) - 1;

/// Opaque codec payload carried by a constructed animation frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FramePayload<'a> {
    Vp8(&'a [u8]),
    /// A VP8L payload. Its fixed header is inspected only for the alpha hint.
    Vp8l(&'a [u8]),
}

/// One animation frame supplied to [`crate::Muxer::add_animation_frame`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnimationFrameInput<'a> {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub duration_ms: u32,
    pub dispose_to_background: bool,
    pub blend: bool,
    pub alpha: Option<&'a [u8]>,
    pub payload: FramePayload<'a>,
}

pub(crate) fn serialize_animation_frame_input(
    frame: AnimationFrameInput<'_>,
) -> Result<Vec<u8>, ContainerError> {
    if !dimensions_fit_u24_minus_one(frame.width, frame.height)
        || frame.x & 1 != 0
        || frame.y & 1 != 0
        || frame.x > 0x01ff_fffe
        || frame.y > 0x01ff_fffe
        || frame.duration_ms > MAX_ANIMATION_DURATION_MS
        || matches!(frame.payload, FramePayload::Vp8l(_)) && frame.alpha.is_some()
    {
        return Err(error(
            ContainerErrorKind::InvalidAnimation,
            "invalid ANMF wire fields",
        ));
    }
    let (fourcc, bitstream) = match frame.payload {
        FramePayload::Vp8(payload) => (VP8, payload),
        FramePayload::Vp8l(payload) => (VP8L, payload),
    };
    let mut nested_size = chunk_storage_len(bitstream.len())?;
    if let Some(alpha) = frame.alpha {
        nested_size = nested_size
            .checked_add(chunk_storage_len(alpha.len())?)
            .ok_or_else(size_overflow)?;
    }
    let payload_len = 16_usize
        .checked_add(nested_size)
        .ok_or_else(size_overflow)?;
    let mut output = reserve(payload_len)?;
    output.extend_from_slice(&(frame.x / 2).to_le_bytes()[..3]);
    output.extend_from_slice(&(frame.y / 2).to_le_bytes()[..3]);
    output.extend_from_slice(&(frame.width - 1).to_le_bytes()[..3]);
    output.extend_from_slice(&(frame.height - 1).to_le_bytes()[..3]);
    output.extend_from_slice(&frame.duration_ms.to_le_bytes()[..3]);
    output.push(u8::from(frame.dispose_to_background) | if frame.blend { 0 } else { 1 << 1 });
    if let Some(alpha) = frame.alpha {
        push_chunk(&mut output, &ALPH, alpha)?;
    }
    push_chunk(&mut output, &fourcc, bitstream)?;
    Ok(output)
}
