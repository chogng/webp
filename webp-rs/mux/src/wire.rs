//! RIFF allocation, chunk framing, and strict finished-output validation.

use crate::CompatibilityProfile;
use crate::ContainerError;
use crate::ContainerErrorKind;
use crate::ContainerLimits;
use crate::DemuxOptions;
use crate::MuxChunk;

const MAX_WIRE_DIMENSION: u32 = 1 << 24;

pub(crate) const fn dimensions_fit_u24_minus_one(width: u32, height: u32) -> bool {
    width != 0 && height != 0 && width <= MAX_WIRE_DIMENSION && height <= MAX_WIRE_DIMENSION
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

pub(crate) fn riff_capacity(chunks_size: usize) -> Result<usize, ContainerError> {
    let riff_size = 4_usize.checked_add(chunks_size).ok_or_else(size_overflow)?;
    u32::try_from(riff_size).map_err(|_| size_overflow())?;
    riff_size.checked_add(8).ok_or_else(size_overflow)
}

pub(crate) fn reserve(capacity: usize) -> Result<Vec<u8>, ContainerError> {
    let mut output = Vec::new();
    output.try_reserve_exact(capacity).map_err(|_| {
        error(
            ContainerErrorKind::AllocationFailed,
            "WebP container allocation failed",
        )
    })?;
    Ok(output)
}

pub(crate) fn copy_bytes(bytes: &[u8]) -> Result<Vec<u8>, ContainerError> {
    let mut output = reserve(bytes.len())?;
    output.extend_from_slice(bytes);
    Ok(output)
}

pub(crate) fn allocation_failed() -> ContainerError {
    error(
        ContainerErrorKind::AllocationFailed,
        "WebP container allocation failed",
    )
}

pub(crate) fn chunk_storage_len(payload_len: usize) -> Result<usize, ContainerError> {
    u32::try_from(payload_len).map_err(|_| size_overflow())?;
    8_usize
        .checked_add(payload_len)
        .and_then(|size| size.checked_add(payload_len & 1))
        .ok_or_else(size_overflow)
}

pub(crate) fn push_chunk(
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

pub(crate) fn size_overflow() -> ContainerError {
    error(
        ContainerErrorKind::SizeOverflow,
        "WebP container size overflow",
    )
}

pub(crate) const fn error(kind: ContainerErrorKind, context: &'static str) -> ContainerError {
    ContainerError::new(kind, context)
}
