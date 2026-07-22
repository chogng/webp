//! WebP VP8 YUV420 source storage and RGB conversion.

use crate::vp8::Vp8EncodeError;

/// Macroblock-aligned VP8 YUV420 source planes prepared from straight RGBA8.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Vp8SourceYuv {
    pub width: u32,
    pub height: u32,
    pub y_stride: usize,
    pub uv_stride: usize,
    pub y: Vec<u8>,
    pub u: Vec<u8>,
    pub v: Vec<u8>,
}

/// Converts straight RGBA8 through SharpYUV into macroblock-aligned VP8 YUV420.
///
/// Alpha is retained by the caller's WebP container policy; the VP8 luma and
/// chroma planes are derived from the straight RGB channels only. SharpYUV owns
/// even-edge sampling, while this module owns final macroblock padding.
pub fn rgba_to_yuv420(
    width: u32,
    height: u32,
    rgba: &[u8],
) -> Result<Vp8SourceYuv, Vp8EncodeError> {
    if width == 0 || height == 0 || width > 0x3fff || height > 0x3fff {
        return Err(Vp8EncodeError::InvalidDimensions);
    }
    let expected = usize::try_from(u64::from(width) * u64::from(height))
        .ok()
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or(Vp8EncodeError::AllocationFailed)?;
    if rgba.len() != expected {
        return Err(Vp8EncodeError::InvalidRgbaLength);
    }
    let macroblock_width =
        usize::try_from(width.div_ceil(16)).map_err(|_| Vp8EncodeError::InvalidDimensions)?;
    let macroblock_height =
        usize::try_from(height.div_ceil(16)).map_err(|_| Vp8EncodeError::InvalidDimensions)?;
    let y_stride = macroblock_width
        .checked_mul(16)
        .ok_or(Vp8EncodeError::AllocationFailed)?;
    let y_height = macroblock_height
        .checked_mul(16)
        .ok_or(Vp8EncodeError::AllocationFailed)?;
    let uv_stride = macroblock_width
        .checked_mul(8)
        .ok_or(Vp8EncodeError::AllocationFailed)?;
    let uv_height = macroblock_height
        .checked_mul(8)
        .ok_or(Vp8EncodeError::AllocationFailed)?;
    let y_len = y_stride
        .checked_mul(y_height)
        .ok_or(Vp8EncodeError::AllocationFailed)?;
    let uv_len = uv_stride
        .checked_mul(uv_height)
        .ok_or(Vp8EncodeError::AllocationFailed)?;
    let mut y = reserve_zeroed(y_len)?;
    let mut u = reserve_zeroed(uv_len)?;
    let mut v = reserve_zeroed(uv_len)?;
    crate::vp8::sharp_yuv::convert_rgba_to_yuv420(
        width,
        height,
        rgba,
        crate::vp8::sharp_yuv::SharpYuvPlanes {
            y_stride,
            uv_stride,
            y: &mut y,
            u: &mut u,
            v: &mut v,
        },
    )?;
    Ok(Vp8SourceYuv {
        width,
        height,
        y_stride,
        uv_stride,
        y,
        u,
        v,
    })
}

pub(super) fn reserve_zeroed(len: usize) -> Result<Vec<u8>, Vp8EncodeError> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(len)
        .map_err(|_| Vp8EncodeError::AllocationFailed)?;
    output.resize(len, 0);
    Ok(output)
}
