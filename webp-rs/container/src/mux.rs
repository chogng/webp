//! Owned RIFF serialization for public muxing and existing encoder adapters.

use crate::ALPH;
use crate::ANIM;
use crate::ANMF;
use crate::CompatibilityProfile;
use crate::ContainerError;
use crate::ContainerErrorKind;
use crate::ContainerLimits;
use crate::DemuxOptions;
use crate::EXIF;
use crate::FourCc;
use crate::ICCP;
use crate::Metadata;
use crate::MuxChunk;
use crate::VP8;
use crate::VP8L;
use crate::VP8X;
use crate::XMP;

const MAX_ANIMATION_DURATION_MS: u32 = (1 << 24) - 1;
const MAX_WIRE_DIMENSION: u32 = 1 << 24;

/// Opaque codec payload carried by a constructed animation frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FramePayload<'a> {
    Vp8(&'a [u8]),
    Vp8l(&'a [u8]),
}

/// One animation frame supplied to [`Muxer::add_animation_frame`].
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

/// Builds a strict WebP RIFF container from owned, opaque chunks.
///
/// Common static and animation constructors establish `VP8X` geometry and
/// flags for callers. [`Muxer::add_chunk`] is also available for extensions
/// and unknown chunks; [`Muxer::finish`] validates the resulting layout.
#[derive(Debug, Default)]
pub struct Muxer {
    chunks: Vec<MuxChunk>,
}

impl Muxer {
    /// Creates an empty generic container builder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates an extended static VP8L container with known canvas geometry.
    pub fn static_vp8l(
        width: u32,
        height: u32,
        payload: Vec<u8>,
        has_alpha: bool,
    ) -> Result<Self, ContainerError> {
        let mut muxer = Self::with_canvas(width, height, u8::from(has_alpha) << 4)?;
        muxer.add_chunk(MuxChunk::new(VP8L, payload))?;
        Ok(muxer)
    }

    /// Creates an extended static VP8 container with an optional ALPH payload.
    pub fn static_vp8(
        width: u32,
        height: u32,
        payload: Vec<u8>,
        alpha: Option<Vec<u8>>,
    ) -> Result<Self, ContainerError> {
        let mut muxer = Self::with_canvas(width, height, u8::from(alpha.is_some()) << 4)?;
        if let Some(alpha) = alpha {
            muxer.add_chunk(MuxChunk::new(ALPH, alpha))?;
        }
        muxer.add_chunk(MuxChunk::new(VP8, payload))?;
        Ok(muxer)
    }

    /// Creates an animated container and writes its `VP8X` and `ANIM` chunks.
    pub fn animation(
        width: u32,
        height: u32,
        background_rgba: [u8; 4],
        loop_count: u16,
    ) -> Result<Self, ContainerError> {
        let mut muxer = Self::with_canvas(width, height, 1 << 1)?;
        let control = [
            background_rgba[2],
            background_rgba[1],
            background_rgba[0],
            background_rgba[3],
            loop_count.to_le_bytes()[0],
            loop_count.to_le_bytes()[1],
        ];
        muxer.add_chunk(MuxChunk::new(ANIM, control.to_vec()))?;
        Ok(muxer)
    }

    /// Adds a top-level opaque chunk. Unknown chunks are serialized unchanged.
    pub fn add_chunk(&mut self, chunk: MuxChunk) -> Result<&mut Self, ContainerError> {
        self.chunks
            .try_reserve(1)
            .map_err(|_| allocation_failed())?;
        self.chunks.push(chunk);
        Ok(self)
    }

    /// Adds an opaque chunk while retaining builder chaining.
    pub fn with_chunk(mut self, chunk: MuxChunk) -> Result<Self, ContainerError> {
        self.add_chunk(chunk)?;
        Ok(self)
    }

    /// Adds a validated ANMF frame to an animated container.
    pub fn add_animation_frame(
        &mut self,
        frame: AnimationFrameInput<'_>,
    ) -> Result<&mut Self, ContainerError> {
        if !self.chunks.iter().any(|chunk| chunk.fourcc() == ANIM) {
            return Err(error(
                ContainerErrorKind::InvalidAnimation,
                "animation frames require Muxer::animation",
            ));
        }
        let payload = serialize_animation_frame_input(frame)?;
        if frame.alpha.is_some() {
            set_vp8x_flag(&mut self.chunks, 1 << 4, true)?;
        }
        self.add_chunk(MuxChunk::new(ANMF, payload))
    }

    /// Replaces or adds ICC profile metadata.
    pub fn set_iccp(&mut self, payload: Vec<u8>) -> Result<&mut Self, ContainerError> {
        set_metadata_chunk(&mut self.chunks, ICCP, payload)?;
        Ok(self)
    }

    /// Replaces or adds EXIF metadata.
    pub fn set_exif(&mut self, payload: Vec<u8>) -> Result<&mut Self, ContainerError> {
        set_metadata_chunk(&mut self.chunks, EXIF, payload)?;
        Ok(self)
    }

    /// Replaces or adds XMP metadata.
    pub fn set_xmp(&mut self, payload: Vec<u8>) -> Result<&mut Self, ContainerError> {
        set_metadata_chunk(&mut self.chunks, XMP, payload)?;
        Ok(self)
    }

    /// Serializes and strictly validates the finished RIFF container.
    pub fn finish(self) -> Result<Vec<u8>, ContainerError> {
        finish_chunks(&self.chunks)
    }

    fn with_canvas(width: u32, height: u32, flags: u8) -> Result<Self, ContainerError> {
        if !dimensions_fit_u24_minus_one(width, height) {
            return Err(error(
                ContainerErrorKind::InvalidDimensions,
                "container dimensions exceed the VP8X wire range",
            ));
        }
        let mut vp8x = [0_u8; 10];
        vp8x[0] = flags;
        vp8x[4..7].copy_from_slice(&(width - 1).to_le_bytes()[..3]);
        vp8x[7..10].copy_from_slice(&(height - 1).to_le_bytes()[..3]);
        let mut muxer = Self::new();
        muxer.add_chunk(MuxChunk::new(VP8X, vp8x.to_vec()))?;
        Ok(muxer)
    }

    pub(crate) fn from_chunks(chunks: Vec<MuxChunk>) -> Self {
        Self { chunks }
    }
}

const fn dimensions_fit_u24_minus_one(width: u32, height: u32) -> bool {
    width != 0 && height != 0 && width <= MAX_WIRE_DIMENSION && height <= MAX_WIRE_DIMENSION
}

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

pub(crate) fn finish_chunks(chunks: &[MuxChunk]) -> Result<Vec<u8>, ContainerError> {
    let mut chunks_size = 0_usize;
    for chunk in chunks {
        chunks_size = chunks_size
            .checked_add(chunk_storage_len(chunk.payload().len())?)
            .ok_or_else(size_overflow)?;
    }
    let capacity = riff_capacity(chunks_size)?;
    let riff_size = u32::try_from(capacity - 8).map_err(|_| size_overflow())?;
    let mut output = reserve(capacity)?;
    output.extend_from_slice(b"RIFF");
    output.extend_from_slice(&riff_size.to_le_bytes());
    output.extend_from_slice(b"WEBP");
    for chunk in chunks {
        push_chunk(&mut output, &chunk.fourcc(), chunk.payload())?;
    }
    let options = DemuxOptions {
        profile: CompatibilityProfile::SpecStrict,
        limits: ContainerLimits {
            max_input_bytes: usize::MAX,
            max_width: MAX_WIRE_DIMENSION,
            max_height: MAX_WIRE_DIMENSION,
            max_pixels: u64::MAX,
            max_frames: u32::MAX,
            max_total_frame_pixels: u64::MAX,
            max_metadata_bytes: usize::MAX,
            max_chunks: u32::MAX,
        },
    };
    crate::Demuxer::parse(&output, &options)?;
    Ok(output)
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
    chunks.remove(index);
    set_vp8x_flag(chunks, flag, false)?;
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
    let mut payload = chunks[vp8x].payload().to_vec();
    if payload.len() != 10 {
        return Err(error(
            ContainerErrorKind::InvalidContainer,
            "VP8X payload must be exactly 10 bytes",
        ));
    }
    if enabled {
        payload[0] |= flag;
    } else {
        payload[0] &= !flag;
    }
    chunks[vp8x] = MuxChunk::new(VP8X, payload);
    Ok(())
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

fn riff_capacity(chunks_size: usize) -> Result<usize, ContainerError> {
    let riff_size = 4_usize.checked_add(chunks_size).ok_or_else(size_overflow)?;
    u32::try_from(riff_size).map_err(|_| size_overflow())?;
    riff_size.checked_add(8).ok_or_else(size_overflow)
}

fn reserve(capacity: usize) -> Result<Vec<u8>, ContainerError> {
    let mut output = Vec::new();
    output.try_reserve_exact(capacity).map_err(|_| {
        error(
            ContainerErrorKind::AllocationFailed,
            "WebP container allocation failed",
        )
    })?;
    Ok(output)
}

pub(crate) fn allocation_failed() -> ContainerError {
    error(
        ContainerErrorKind::AllocationFailed,
        "WebP container allocation failed",
    )
}

fn chunk_storage_len(payload_len: usize) -> Result<usize, ContainerError> {
    u32::try_from(payload_len).map_err(|_| size_overflow())?;
    8_usize
        .checked_add(payload_len)
        .and_then(|size| size.checked_add(payload_len & 1))
        .ok_or_else(size_overflow)
}

fn push_chunk(
    output: &mut Vec<u8>,
    fourcc: &[u8; 4],
    payload: &[u8],
) -> Result<(), ContainerError> {
    let payload_len = u32::try_from(payload.len()).map_err(|_| size_overflow())?;
    output.extend_from_slice(fourcc);
    output.extend_from_slice(&payload_len.to_le_bytes());
    output.extend_from_slice(payload);
    if payload.len() & 1 != 0 {
        output.push(0);
    }
    Ok(())
}

fn size_overflow() -> ContainerError {
    error(
        ContainerErrorKind::SizeOverflow,
        "WebP container size overflow",
    )
}

const fn error(kind: ContainerErrorKind, context: &'static str) -> ContainerError {
    ContainerError::new(kind, context)
}

#[cfg(test)]
#[path = "mux_tests.rs"]
mod tests;
