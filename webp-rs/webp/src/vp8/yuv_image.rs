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

/// Converts straight RGBA8 into edge-replicated, macroblock-aligned VP8 YUV420.
///
/// Alpha is retained by the caller's WebP container policy; the VP8 luma and
/// chroma planes are derived from the straight RGB channels only.
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
    let source_width = usize::try_from(width).map_err(|_| Vp8EncodeError::InvalidDimensions)?;
    let source_height = usize::try_from(height).map_err(|_| Vp8EncodeError::InvalidDimensions)?;
    for row in 0..uv_height {
        for column in 0..uv_stride {
            let mut totals = [0_u16; 3];
            for y_offset in 0..2 {
                for x_offset in 0..2 {
                    let y_row = row * 2 + y_offset;
                    let y_column = column * 2 + x_offset;
                    let [red, green, blue] =
                        rgb_at(rgba, source_width, source_height, y_column, y_row);
                    y[y_row * y_stride + y_column] = rgb_to_y(red, green, blue);
                    totals[0] += u16::from(red);
                    totals[1] += u16::from(green);
                    totals[2] += u16::from(blue);
                }
            }
            let red = ((totals[0] + 2) / 4) as u8;
            let green = ((totals[1] + 2) / 4) as u8;
            let blue = ((totals[2] + 2) / 4) as u8;
            let index = row * uv_stride + column;
            u[index] = rgb_to_u(red, green, blue);
            v[index] = rgb_to_v(red, green, blue);
        }
    }
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

fn rgb_at(rgba: &[u8], width: usize, height: usize, x: usize, y: usize) -> [u8; 3] {
    let x = x.min(width - 1);
    let y = y.min(height - 1);
    let offset = (y * width + x) * 4;
    [rgba[offset], rgba[offset + 1], rgba[offset + 2]]
}

const fn rgb_to_y(red: u8, green: u8, blue: u8) -> u8 {
    (((66 * red as u32 + 129 * green as u32 + 25 * blue as u32 + 128) >> 8) + 16) as u8
}

const fn rgb_to_u(red: u8, green: u8, blue: u8) -> u8 {
    (((-38 * red as i32 - 74 * green as i32 + 112 * blue as i32 + 128) >> 8) + 128) as u8
}

const fn rgb_to_v(red: u8, green: u8, blue: u8) -> u8 {
    (((112 * red as i32 - 94 * green as i32 - 18 * blue as i32 + 128) >> 8) + 128) as u8
}
