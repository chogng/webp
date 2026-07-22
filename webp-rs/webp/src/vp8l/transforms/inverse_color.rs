use crate::vp8l::header::BlockTransformDescriptor;
use crate::vp8l::transforms::color::ColorTransformMultipliers;
use webp_core::DecodeError;
use webp_core::DecodeErrorKind;

/// Inverts subtract-green directly in the packed ARGB representation.
///
/// Keeping this as a packed-pixel helper avoids allocating a second image
/// buffer solely to adapt to the transform crate's RGBA image type. The green
/// and alpha lanes are unchanged, while red and blue add green modulo 256.
pub(in crate::vp8l) fn inverse_subtract_green_argb(pixels: &mut [u32]) {
    for pixel in pixels {
        let green = (*pixel >> 8) as u8;
        let red = ((*pixel >> 16) as u8).wrapping_add(green);
        let blue = (*pixel as u8).wrapping_add(green);
        *pixel = (*pixel & 0xff00_ff00) | (u32::from(red) << 16) | u32::from(blue);
    }
}

/// Inverts a color transform in packed ARGB order without a second image
/// buffer.  The coefficient table has already been validated against the
/// descriptor during transform-subimage decoding.
pub(in crate::vp8l) fn inverse_color_argb(
    pixels: &mut [u32],
    descriptor: BlockTransformDescriptor,
    multipliers: &[ColorTransformMultipliers],
) -> Result<(), DecodeError> {
    let width = usize::try_from(descriptor.image_width).map_err(|_| {
        DecodeError::new(
            DecodeErrorKind::LimitExceeded,
            None,
            "VP8L color-transform image width does not fit usize",
        )
    })?;
    let height = usize::try_from(descriptor.image_height).map_err(|_| {
        DecodeError::new(
            DecodeErrorKind::LimitExceeded,
            None,
            "VP8L color-transform image height does not fit usize",
        )
    })?;
    let expected_pixels = width.checked_mul(height).ok_or_else(|| {
        DecodeError::new(
            DecodeErrorKind::LimitExceeded,
            None,
            "VP8L color-transform image pixel count overflow",
        )
    })?;
    if pixels.len() != expected_pixels {
        return Err(DecodeError::new(
            DecodeErrorKind::InvalidBitstream,
            None,
            "VP8L color-transform output length does not match image dimensions",
        ));
    }

    let table_width = usize::try_from(descriptor.transform_width).map_err(|_| {
        DecodeError::new(
            DecodeErrorKind::LimitExceeded,
            None,
            "VP8L color-transform table width does not fit usize",
        )
    })?;
    let table_height = usize::try_from(descriptor.transform_height).map_err(|_| {
        DecodeError::new(
            DecodeErrorKind::LimitExceeded,
            None,
            "VP8L color-transform table height does not fit usize",
        )
    })?;
    let expected_multipliers = table_width.checked_mul(table_height).ok_or_else(|| {
        DecodeError::new(
            DecodeErrorKind::LimitExceeded,
            None,
            "VP8L color-transform table pixel count overflow",
        )
    })?;
    if multipliers.len() != expected_multipliers {
        return Err(DecodeError::new(
            DecodeErrorKind::InvalidBitstream,
            None,
            "VP8L color-transform table has unexpected dimensions",
        ));
    }

    let block_size = usize::try_from(descriptor.block_size()).map_err(|_| {
        DecodeError::new(
            DecodeErrorKind::LimitExceeded,
            None,
            "VP8L color-transform block size does not fit usize",
        )
    })?;
    for (y, row) in pixels.chunks_exact_mut(width).enumerate() {
        let table_row = (y / block_size) * table_width;
        let row_multipliers = &multipliers[table_row..table_row + table_width];
        for (block, &multiplier) in row.chunks_mut(block_size).zip(row_multipliers) {
            for pixel in block {
                *pixel = inverse_color_pixel_argb(*pixel, multiplier);
            }
        }
    }
    Ok(())
}

/// Applies libwebp's scalar VP8L inverse color arithmetic to one packed pixel.
/// Both green and the reconstructed red fed to the blue multiplier are signed
/// bytes; the red result is reduced modulo 256 before the final multiplication.
const fn inverse_color_pixel_argb(pixel: u32, multipliers: ColorTransformMultipliers) -> u32 {
    let green = ((pixel >> 8) as u8) as i8;
    let mut red = (pixel >> 16) as u8 as i32;
    let mut blue = pixel as u8 as i32;
    red = (red + color_delta(multipliers.green_to_red, green)) & 0xff;
    blue += color_delta(multipliers.green_to_blue, green);
    blue += color_delta(multipliers.red_to_blue, red as u8 as i8);
    blue &= 0xff;
    (pixel & 0xff00_ff00) | ((red as u32) << 16) | (blue as u32)
}

const fn color_delta(multiplier: i8, channel: i8) -> i32 {
    ((multiplier as i32) * (channel as i32)) >> 5
}

#[cfg(test)]
#[path = "inverse_color_tests.rs"]
mod tests;
