//! Compatibility serializers used by the encoder crates.

use crate::ContainerError;
use crate::ContainerErrorKind;
use crate::Metadata;
use crate::frame::AnimationFrameInput;
use crate::frame::FramePayload;
use crate::frame::serialize_animation_frame_input;
use crate::wire::chunk_storage_len;
use crate::wire::dimensions_fit_u24_minus_one;
use crate::wire::error;
use crate::wire::push_chunk;
use crate::wire::reserve;
use crate::wire::riff_capacity;
use crate::wire::size_overflow;

/// Opaque VP8L frame payload and its existing ANMF wire fields.
#[doc(hidden)]
#[derive(Debug, Clone, Copy)]
pub struct AnimationFrameMux<'a> {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub duration_ms: u32,
    pub dispose_to_background: bool,
    pub blend: bool,
    pub vp8l_payload: &'a [u8],
}

/// Existing animation-control fields needed by the public `webp` encoder.
#[doc(hidden)]
#[derive(Debug, Clone, Copy)]
pub struct AnimationMuxOptions {
    pub background_rgba: [u8; 4],
    pub loop_count: u16,
}

/// Serializes the existing static VP8L container profile.
#[doc(hidden)]
pub fn serialize_vp8l(
    payload: Vec<u8>,
    width: u32,
    height: u32,
    has_alpha: bool,
    metadata: Metadata<'_>,
) -> Result<Vec<u8>, ContainerError> {
    let has_metadata = metadata.iccp.is_some() || metadata.exif.is_some() || metadata.xmp.is_some();
    if !has_metadata {
        return wrap_vp8l_chunks(payload, None, None, None, None);
    }
    if !dimensions_fit_u24_minus_one(width, height) {
        return Err(error(
            ContainerErrorKind::InvalidDimensions,
            "extended VP8L container dimensions exceed the VP8X wire range",
        ));
    }

    let mut flags = 0_u8;
    if metadata.iccp.is_some() {
        flags |= 1 << 5;
    }
    if has_alpha {
        flags |= 1 << 4;
    }
    if metadata.exif.is_some() {
        flags |= 1 << 3;
    }
    if metadata.xmp.is_some() {
        flags |= 1 << 2;
    }
    let mut vp8x = [0_u8; 10];
    vp8x[0] = flags;
    vp8x[4..7].copy_from_slice(&(width - 1).to_le_bytes()[..3]);
    vp8x[7..10].copy_from_slice(&(height - 1).to_le_bytes()[..3]);
    wrap_vp8l_chunks(
        payload,
        Some(&vp8x),
        metadata.iccp,
        metadata.exif,
        metadata.xmp,
    )
}

/// Serializes the existing static VP8 container profile.
#[doc(hidden)]
pub fn serialize_vp8(
    payload: Vec<u8>,
    width: u32,
    height: u32,
    alpha: Option<&[u8]>,
) -> Result<Vec<u8>, ContainerError> {
    let mut chunks_size = chunk_storage_len(payload.len())?;
    if let Some(alpha) = alpha {
        chunks_size = chunks_size
            .checked_add(chunk_storage_len(10)?)
            .and_then(|size| size.checked_add(chunk_storage_len(alpha.len()).ok()?))
            .ok_or_else(size_overflow)?;
    }
    let capacity = riff_capacity(chunks_size)?;
    let riff_size = u32::try_from(capacity - 8).map_err(|_| size_overflow())?;
    let mut output = reserve(capacity)?;
    output.extend_from_slice(b"RIFF");
    output.extend_from_slice(&riff_size.to_le_bytes());
    output.extend_from_slice(b"WEBP");
    if let Some(alpha) = alpha {
        if !dimensions_fit_u24_minus_one(width, height) {
            return Err(error(
                ContainerErrorKind::InvalidDimensions,
                "extended VP8 container dimensions exceed the VP8X wire range",
            ));
        }
        let mut vp8x = [0_u8; 10];
        vp8x[0] = 1 << 4;
        vp8x[4..7].copy_from_slice(&(width - 1).to_le_bytes()[..3]);
        vp8x[7..10].copy_from_slice(&(height - 1).to_le_bytes()[..3]);
        push_chunk(&mut output, b"VP8X", &vp8x)?;
        push_chunk(&mut output, b"ALPH", alpha)?;
    }
    push_chunk(&mut output, b"VP8 ", &payload)?;
    Ok(output)
}

/// Serializes one existing VP8L ANMF payload.
#[doc(hidden)]
pub fn serialize_animation_frame(frame: AnimationFrameMux<'_>) -> Result<Vec<u8>, ContainerError> {
    serialize_animation_frame_input(AnimationFrameInput {
        x: frame.x,
        y: frame.y,
        width: frame.width,
        height: frame.height,
        duration_ms: frame.duration_ms,
        dispose_to_background: frame.dispose_to_background,
        blend: frame.blend,
        alpha: None,
        payload: FramePayload::Vp8l(frame.vp8l_payload),
    })
}

/// Serializes the existing VP8L-frame animation container profile.
#[doc(hidden)]
pub fn serialize_animation(
    width: u32,
    height: u32,
    options: AnimationMuxOptions,
    has_alpha: bool,
    frames: &[Vec<u8>],
    metadata: Metadata<'_>,
) -> Result<Vec<u8>, ContainerError> {
    if !dimensions_fit_u24_minus_one(width, height) {
        return Err(error(
            ContainerErrorKind::InvalidDimensions,
            "animation canvas dimensions exceed the VP8X wire range",
        ));
    }
    let mut chunks_size = chunk_storage_len(10)?;
    chunks_size = chunks_size
        .checked_add(chunk_storage_len(6)?)
        .ok_or_else(size_overflow)?;
    for value in [metadata.iccp, metadata.exif, metadata.xmp]
        .into_iter()
        .flatten()
    {
        chunks_size = chunks_size
            .checked_add(chunk_storage_len(value.len())?)
            .ok_or_else(size_overflow)?;
    }
    for frame in frames {
        chunks_size = chunks_size
            .checked_add(chunk_storage_len(frame.len())?)
            .ok_or_else(size_overflow)?;
    }
    let capacity = riff_capacity(chunks_size)?;
    let riff_size = u32::try_from(capacity - 8).map_err(|_| size_overflow())?;
    let mut output = reserve(capacity)?;

    let mut vp8x = [0_u8; 10];
    vp8x[0] = (1 << 1)
        | if has_alpha { 1 << 4 } else { 0 }
        | if metadata.iccp.is_some() { 1 << 5 } else { 0 }
        | if metadata.exif.is_some() { 1 << 3 } else { 0 }
        | if metadata.xmp.is_some() { 1 << 2 } else { 0 };
    vp8x[4..7].copy_from_slice(&(width - 1).to_le_bytes()[..3]);
    vp8x[7..10].copy_from_slice(&(height - 1).to_le_bytes()[..3]);
    let animation_control = [
        options.background_rgba[2],
        options.background_rgba[1],
        options.background_rgba[0],
        options.background_rgba[3],
        options.loop_count.to_le_bytes()[0],
        options.loop_count.to_le_bytes()[1],
    ];
    output.extend_from_slice(b"RIFF");
    output.extend_from_slice(&riff_size.to_le_bytes());
    output.extend_from_slice(b"WEBP");
    push_chunk(&mut output, b"VP8X", &vp8x)?;
    if let Some(iccp) = metadata.iccp {
        push_chunk(&mut output, b"ICCP", iccp)?;
    }
    push_chunk(&mut output, b"ANIM", &animation_control)?;
    for frame in frames {
        push_chunk(&mut output, b"ANMF", frame)?;
    }
    if let Some(exif) = metadata.exif {
        push_chunk(&mut output, b"EXIF", exif)?;
    }
    if let Some(xmp) = metadata.xmp {
        push_chunk(&mut output, b"XMP ", xmp)?;
    }
    Ok(output)
}

fn wrap_vp8l_chunks(
    payload: Vec<u8>,
    vp8x: Option<&[u8; 10]>,
    iccp: Option<&[u8]>,
    exif: Option<&[u8]>,
    xmp: Option<&[u8]>,
) -> Result<Vec<u8>, ContainerError> {
    let mut chunks_size = chunk_storage_len(payload.len())?;
    for value in [vp8x.map(|value| value.as_slice()), iccp, exif, xmp]
        .into_iter()
        .flatten()
    {
        chunks_size = chunks_size
            .checked_add(chunk_storage_len(value.len())?)
            .ok_or_else(size_overflow)?;
    }
    let capacity = riff_capacity(chunks_size)?;
    let riff_size = u32::try_from(capacity - 8).map_err(|_| size_overflow())?;
    let mut output = reserve(capacity)?;
    output.extend_from_slice(b"RIFF");
    output.extend_from_slice(&riff_size.to_le_bytes());
    output.extend_from_slice(b"WEBP");
    if let Some(vp8x) = vp8x {
        push_chunk(&mut output, b"VP8X", vp8x)?;
    }
    if let Some(iccp) = iccp {
        push_chunk(&mut output, b"ICCP", iccp)?;
    }
    push_chunk(&mut output, b"VP8L", &payload)?;
    if let Some(exif) = exif {
        push_chunk(&mut output, b"EXIF", exif)?;
    }
    if let Some(xmp) = xmp {
        push_chunk(&mut output, b"XMP ", xmp)?;
    }
    Ok(output)
}
