//! Fixed VP8 and VP8L image-header validation.
//!
//! Demuxing does not decode pixels, but the fixed headers are container-facing:
//! they establish canvas dimensions, alpha hints, and whether an opaque image
//! payload is structurally usable as a WebP frame.

use crate::ContainerError;
use crate::ContainerErrorKind;
use crate::ContainerLimits;
use crate::FourCc;
use crate::VP8;
use crate::VP8L;

const VP8_KEY_FRAME_HEADER_LEN: usize = 10;
const VP8_KEY_FRAME_START_CODE: [u8; 3] = [0x9d, 0x01, 0x2a];
const VP8L_HEADER_LEN: usize = 5;
const VP8L_SIGNATURE: u8 = 0x2f;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ImageHeader {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) alpha_hint: bool,
}

pub(crate) fn parse(
    fourcc: FourCc,
    payload: &[u8],
    limits: &ContainerLimits,
    payload_offset: usize,
) -> Result<ImageHeader, ContainerError> {
    match fourcc {
        VP8 => parse_vp8(payload, limits, payload_offset),
        VP8L => parse_vp8l(payload, limits, payload_offset),
        _ => Err(error(
            ContainerErrorKind::InvalidContainer,
            payload_offset,
            "image header parser requires VP8 or VP8L",
        )),
    }
}

fn parse_vp8(
    payload: &[u8],
    limits: &ContainerLimits,
    payload_offset: usize,
) -> Result<ImageHeader, ContainerError> {
    if payload.len() < VP8_KEY_FRAME_HEADER_LEN {
        return Err(error(
            ContainerErrorKind::UnexpectedEof,
            payload_offset + payload.len(),
            "truncated VP8 key-frame header",
        ));
    }
    let frame_tag =
        u32::from(payload[0]) | (u32::from(payload[1]) << 8) | (u32::from(payload[2]) << 16);
    if frame_tag & 1 != 0 {
        return Err(error(
            ContainerErrorKind::InvalidContainer,
            payload_offset,
            "WebP VP8 payload must be a key frame",
        ));
    }
    let version = (frame_tag >> 1) & 0x7;
    if version > 3 || frame_tag & (1 << 4) == 0 {
        return Err(error(
            ContainerErrorKind::InvalidContainer,
            payload_offset,
            "invalid WebP VP8 key-frame tag",
        ));
    }
    let first_partition_len = usize::try_from(frame_tag >> 5).map_err(|_| {
        error(
            ContainerErrorKind::SizeOverflow,
            payload_offset,
            "VP8 first partition length does not fit usize",
        )
    })?;
    if first_partition_len > payload.len() - VP8_KEY_FRAME_HEADER_LEN {
        return Err(error(
            ContainerErrorKind::UnexpectedEof,
            payload_offset + 3,
            "VP8 first partition exceeds payload",
        ));
    }
    if payload[3..6] != VP8_KEY_FRAME_START_CODE {
        return Err(error(
            ContainerErrorKind::InvalidContainer,
            payload_offset + 3,
            "invalid VP8 key-frame start code",
        ));
    }
    let width = u32::from(u16::from_le_bytes([payload[6], payload[7]]) & 0x3fff);
    let height = u32::from(u16::from_le_bytes([payload[8], payload[9]]) & 0x3fff);
    check_dimensions(width, height, limits, payload_offset + 6)?;
    Ok(ImageHeader {
        width,
        height,
        alpha_hint: false,
    })
}

fn parse_vp8l(
    payload: &[u8],
    limits: &ContainerLimits,
    payload_offset: usize,
) -> Result<ImageHeader, ContainerError> {
    if payload.len() < VP8L_HEADER_LEN {
        return Err(error(
            ContainerErrorKind::UnexpectedEof,
            payload_offset + payload.len(),
            "truncated VP8L header",
        ));
    }
    if payload[0] != VP8L_SIGNATURE {
        return Err(error(
            ContainerErrorKind::InvalidContainer,
            payload_offset,
            "invalid VP8L signature",
        ));
    }
    let fields = u32::from_le_bytes(payload[1..5].try_into().expect("fixed VP8L header"));
    let width = (fields & 0x3fff) + 1;
    let height = ((fields >> 14) & 0x3fff) + 1;
    let alpha_hint = fields & (1 << 28) != 0;
    if fields >> 29 != 0 {
        return Err(error(
            ContainerErrorKind::InvalidContainer,
            payload_offset + 4,
            "unsupported VP8L version",
        ));
    }
    check_dimensions(width, height, limits, payload_offset + 1)?;
    Ok(ImageHeader {
        width,
        height,
        alpha_hint,
    })
}

fn check_dimensions(
    width: u32,
    height: u32,
    limits: &ContainerLimits,
    offset: usize,
) -> Result<(), ContainerError> {
    if width == 0 || height == 0 {
        return Err(error(
            ContainerErrorKind::InvalidDimensions,
            offset,
            "image dimensions must be non-zero",
        ));
    }
    if width > limits.max_width || height > limits.max_height {
        return Err(error(
            ContainerErrorKind::LimitExceeded,
            offset,
            "image dimension exceeds limit",
        ));
    }
    let pixels = u64::from(width) * u64::from(height);
    if pixels > limits.max_pixels {
        return Err(error(
            ContainerErrorKind::LimitExceeded,
            offset,
            "image pixels exceed limit",
        ));
    }
    Ok(())
}

fn error(kind: ContainerErrorKind, offset: usize, context: &'static str) -> ContainerError {
    ContainerError::at(kind, offset, context)
}

#[cfg(test)]
#[path = "image_header_tests.rs"]
mod tests;
