//! Owned chunk-sequence mutations and their container invariants.

use crate::ALPH;
use crate::ANIM;
use crate::ANMF;
use crate::ContainerError;
use crate::ContainerErrorKind;
use crate::EXIF;
use crate::FourCc;
use crate::ICCP;
use crate::MuxChunk;
use crate::VP8;
use crate::VP8L;
use crate::VP8X;
use crate::XMP;
use crate::frame::AnimationFrameInput;
use crate::frame::FramePayload;
use crate::frame::serialize_animation_frame_input;
use crate::wire::allocation_failed;
use crate::wire::copy_bytes;
use crate::wire::dimensions_fit_u24_minus_one;
use crate::wire::error;

pub(crate) fn insert_chunk(
    chunks: &mut Vec<MuxChunk>,
    index: usize,
    chunk: MuxChunk,
) -> Result<(), ContainerError> {
    if index > chunks.len() {
        return Err(error(
            ContainerErrorKind::InvalidContainer,
            "chunk insertion index is out of bounds",
        ));
    }
    chunks.try_reserve(1).map_err(|_| allocation_failed())?;
    chunks.insert(index, chunk);
    Ok(())
}

pub(crate) fn replace_chunk(
    chunks: &mut [MuxChunk],
    index: usize,
    chunk: MuxChunk,
) -> Option<MuxChunk> {
    chunks
        .get_mut(index)
        .map(|current| core::mem::replace(current, chunk))
}

pub(crate) fn remove_chunk(chunks: &mut Vec<MuxChunk>, index: usize) -> Option<MuxChunk> {
    (index < chunks.len()).then(|| chunks.remove(index))
}

pub(crate) fn insert_animation_frame(
    chunks: &mut Vec<MuxChunk>,
    frame_index: usize,
    frame: AnimationFrameInput<'_>,
) -> Result<(), ContainerError> {
    let anim = chunks
        .iter()
        .position(|chunk| chunk.fourcc() == ANIM)
        .ok_or_else(|| {
            error(
                ContainerErrorKind::InvalidAnimation,
                "animation frames require an ANIM chunk",
            )
        })?;
    let frame_count = chunks.iter().filter(|chunk| chunk.fourcc() == ANMF).count();
    if frame_index > frame_count {
        return Err(error(
            ContainerErrorKind::InvalidAnimation,
            "animation frame insertion index is out of bounds",
        ));
    }
    let insert_at = if frame_index == frame_count {
        chunks
            .iter()
            .enumerate()
            .rev()
            .find(|(_, chunk)| chunk.fourcc() == ANMF)
            .map_or(anim + 1, |(index, _)| index + 1)
    } else {
        chunks
            .iter()
            .enumerate()
            .filter(|(_, chunk)| chunk.fourcc() == ANMF)
            .nth(frame_index)
            .map(|(index, _)| index)
            .expect("validated animation frame index")
    };
    let payload = serialize_animation_frame_input(frame)?;
    chunks.try_reserve(1).map_err(|_| allocation_failed())?;
    if frame_uses_alpha(frame) {
        set_vp8x_flag(chunks, 1 << 4, true)?;
    }
    chunks.insert(insert_at, MuxChunk::new(ANMF, payload));
    Ok(())
}

pub(crate) fn remove_animation_frame(
    chunks: &mut Vec<MuxChunk>,
    frame_index: usize,
) -> Option<MuxChunk> {
    let index = chunks
        .iter()
        .enumerate()
        .filter(|(_, chunk)| chunk.fourcc() == ANMF)
        .nth(frame_index)
        .map(|(index, _)| index)?;
    Some(chunks.remove(index))
}

pub(crate) fn set_animation_params(
    chunks: &mut Vec<MuxChunk>,
    background_rgba: [u8; 4],
    loop_count: u16,
) -> Result<(), ContainerError> {
    let control = [
        background_rgba[2],
        background_rgba[1],
        background_rgba[0],
        background_rgba[3],
        loop_count.to_le_bytes()[0],
        loop_count.to_le_bytes()[1],
    ];
    let payload = copy_bytes(&control)?;
    if let Some(index) = chunks.iter().position(|chunk| chunk.fourcc() == ANIM) {
        set_vp8x_flag(chunks, 1 << 1, true)?;
        chunks[index] = MuxChunk::new(ANIM, payload);
        return Ok(());
    }
    chunks.try_reserve(1).map_err(|_| allocation_failed())?;
    set_vp8x_flag(chunks, 1 << 1, true)?;
    let vp8x = chunks
        .iter()
        .position(|chunk| chunk.fourcc() == VP8X)
        .expect("VP8X is required by set_vp8x_flag");
    let insert_at = chunks
        .iter()
        .enumerate()
        .skip(vp8x + 1)
        .take_while(|(_, chunk)| chunk.fourcc() == ICCP)
        .last()
        .map_or(vp8x + 1, |(index, _)| index + 1);
    chunks.insert(insert_at, MuxChunk::new(ANIM, payload));
    Ok(())
}

pub(crate) fn set_canvas_size(
    chunks: &mut [MuxChunk],
    width: u32,
    height: u32,
) -> Result<(), ContainerError> {
    if !dimensions_fit_u24_minus_one(width, height) {
        return Err(error(
            ContainerErrorKind::InvalidDimensions,
            "container dimensions exceed the VP8X wire range",
        ));
    }
    let vp8x = chunks
        .iter()
        .position(|chunk| chunk.fourcc() == VP8X)
        .ok_or_else(|| {
            error(
                ContainerErrorKind::InvalidContainer,
                "canvas dimensions require a VP8X chunk",
            )
        })?;
    let mut payload: [u8; 10] = chunks[vp8x].payload().try_into().map_err(|_| {
        error(
            ContainerErrorKind::InvalidContainer,
            "VP8X payload must be exactly 10 bytes",
        )
    })?;
    payload[4..7].copy_from_slice(&(width - 1).to_le_bytes()[..3]);
    payload[7..10].copy_from_slice(&(height - 1).to_le_bytes()[..3]);
    chunks[vp8x] = MuxChunk::new(VP8X, copy_bytes(&payload)?);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn set_static_image(
    chunks: &mut Vec<MuxChunk>,
    width: u32,
    height: u32,
    image_fourcc: FourCc,
    payload: Vec<u8>,
    alpha: Option<Vec<u8>>,
    has_alpha: bool,
) -> Result<(), ContainerError> {
    if !dimensions_fit_u24_minus_one(width, height) {
        return Err(error(
            ContainerErrorKind::InvalidDimensions,
            "container dimensions exceed the VP8X wire range",
        ));
    }
    let mut flags = u8::from(has_alpha) << 4;
    if chunks.iter().any(|chunk| chunk.fourcc() == ICCP) {
        flags |= 1 << 5;
    }
    if chunks.iter().any(|chunk| chunk.fourcc() == EXIF) {
        flags |= 1 << 3;
    }
    if chunks.iter().any(|chunk| chunk.fourcc() == XMP) {
        flags |= 1 << 2;
    }
    let vp8x = make_vp8x(width, height, flags)?;
    let mut rebuilt = Vec::new();
    rebuilt
        .try_reserve_exact(chunks.len().saturating_add(2))
        .map_err(|_| allocation_failed())?;
    rebuilt.push(MuxChunk::new(VP8X, vp8x));
    let mut image = Some(MuxChunk::new(image_fourcc, payload));
    let mut alpha = alpha.map(|payload| MuxChunk::new(ALPH, payload));
    for chunk in core::mem::take(chunks) {
        if matches!(chunk.fourcc(), VP8X | VP8 | VP8L | ALPH | ANIM | ANMF) {
            continue;
        }
        if matches!(chunk.fourcc(), EXIF | XMP) {
            push_static_chunks(&mut rebuilt, &mut alpha, &mut image);
        }
        rebuilt.push(chunk);
    }
    push_static_chunks(&mut rebuilt, &mut alpha, &mut image);
    *chunks = rebuilt;
    Ok(())
}

pub(crate) fn set_animation(
    chunks: &mut Vec<MuxChunk>,
    width: u32,
    height: u32,
    background_rgba: [u8; 4],
    loop_count: u16,
) -> Result<(), ContainerError> {
    if !dimensions_fit_u24_minus_one(width, height) {
        return Err(error(
            ContainerErrorKind::InvalidDimensions,
            "container dimensions exceed the VP8X wire range",
        ));
    }
    let mut flags = 1 << 1;
    if chunks.iter().any(|chunk| chunk.fourcc() == ICCP) {
        flags |= 1 << 5;
    }
    if chunks.iter().any(|chunk| chunk.fourcc() == EXIF) {
        flags |= 1 << 3;
    }
    if chunks.iter().any(|chunk| chunk.fourcc() == XMP) {
        flags |= 1 << 2;
    }
    let vp8x = make_vp8x(width, height, flags)?;
    let control = [
        background_rgba[2],
        background_rgba[1],
        background_rgba[0],
        background_rgba[3],
        loop_count.to_le_bytes()[0],
        loop_count.to_le_bytes()[1],
    ];
    let control = copy_bytes(&control)?;
    let mut rebuilt = Vec::new();
    rebuilt
        .try_reserve_exact(chunks.len().saturating_add(1))
        .map_err(|_| allocation_failed())?;
    rebuilt.push(MuxChunk::new(VP8X, vp8x));
    let mut anim = Some(MuxChunk::new(ANIM, control));
    for chunk in core::mem::take(chunks) {
        if matches!(chunk.fourcc(), VP8X | VP8 | VP8L | ALPH | ANIM | ANMF) {
            continue;
        }
        if chunk.fourcc() != ICCP && anim.is_some() {
            rebuilt.push(anim.take().expect("checked animation controls"));
        }
        rebuilt.push(chunk);
    }
    if let Some(anim) = anim {
        rebuilt.push(anim);
    }
    *chunks = rebuilt;
    Ok(())
}

fn push_static_chunks(
    chunks: &mut Vec<MuxChunk>,
    alpha: &mut Option<MuxChunk>,
    image: &mut Option<MuxChunk>,
) {
    if let Some(alpha) = alpha.take() {
        chunks.push(alpha);
    }
    if let Some(image) = image.take() {
        chunks.push(image);
    }
}

fn make_vp8x(width: u32, height: u32, flags: u8) -> Result<Vec<u8>, ContainerError> {
    let mut vp8x = [0_u8; 10];
    vp8x[0] = flags;
    vp8x[4..7].copy_from_slice(&(width - 1).to_le_bytes()[..3]);
    vp8x[7..10].copy_from_slice(&(height - 1).to_le_bytes()[..3]);
    copy_bytes(&vp8x)
}

pub(crate) fn frame_uses_alpha(frame: AnimationFrameInput<'_>) -> bool {
    frame.alpha.is_some()
        || match frame.payload {
            FramePayload::Vp8(_) => false,
            FramePayload::Vp8l(payload) => vp8l_alpha_hint(payload),
        }
}

fn vp8l_alpha_hint(payload: &[u8]) -> bool {
    payload.first() == Some(&0x2f)
        && payload
            .get(1..5)
            .and_then(|fields| fields.try_into().ok())
            .map(u32::from_le_bytes)
            .is_some_and(|fields| fields & (1 << 28) != 0)
}

pub(crate) fn set_metadata_chunk(
    chunks: &mut Vec<MuxChunk>,
    fourcc: FourCc,
    payload: Vec<u8>,
) -> Result<(), ContainerError> {
    let flag = metadata_flag(fourcc).ok_or_else(|| {
        error(
            ContainerErrorKind::InvalidContainer,
            "chunk is not editable metadata",
        )
    })?;
    if let Some(index) = chunks.iter().position(|chunk| chunk.fourcc() == fourcc) {
        set_vp8x_flag(chunks, flag, true)?;
        chunks[index] = MuxChunk::new(fourcc, payload);
        return Ok(());
    }
    chunks.try_reserve(1).map_err(|_| allocation_failed())?;
    set_vp8x_flag(chunks, flag, true)?;
    let index = if fourcc == ICCP {
        chunks
            .iter()
            .position(|chunk| chunk.fourcc() == VP8X)
            .expect("VP8X is required by set_vp8x_flag")
            + 1
    } else {
        chunks.len()
    };
    chunks.insert(index, MuxChunk::new(fourcc, payload));
    Ok(())
}

pub(crate) fn remove_metadata_chunk(
    chunks: &mut Vec<MuxChunk>,
    fourcc: FourCc,
) -> Result<bool, ContainerError> {
    let flag = metadata_flag(fourcc).ok_or_else(|| {
        error(
            ContainerErrorKind::InvalidContainer,
            "chunk is not editable metadata",
        )
    })?;
    let Some(index) = chunks.iter().position(|chunk| chunk.fourcc() == fourcc) else {
        return Ok(false);
    };
    set_vp8x_flag(chunks, flag, false)?;
    chunks.remove(index);
    Ok(true)
}

fn metadata_flag(fourcc: FourCc) -> Option<u8> {
    match fourcc {
        ICCP => Some(1 << 5),
        EXIF => Some(1 << 3),
        XMP => Some(1 << 2),
        _ => None,
    }
}

pub(crate) fn set_vp8x_flag(
    chunks: &mut [MuxChunk],
    flag: u8,
    enabled: bool,
) -> Result<(), ContainerError> {
    let vp8x = chunks
        .iter()
        .position(|chunk| chunk.fourcc() == VP8X)
        .ok_or_else(|| {
            error(
                ContainerErrorKind::InvalidContainer,
                "metadata and alpha require a VP8X chunk",
            )
        })?;
    let mut payload: [u8; 10] = chunks[vp8x].payload().try_into().map_err(|_| {
        error(
            ContainerErrorKind::InvalidContainer,
            "VP8X payload must be exactly 10 bytes",
        )
    })?;
    if enabled {
        payload[0] |= flag;
    } else {
        payload[0] &= !flag;
    }
    chunks[vp8x] = MuxChunk::new(VP8X, copy_bytes(&payload)?);
    Ok(())
}
