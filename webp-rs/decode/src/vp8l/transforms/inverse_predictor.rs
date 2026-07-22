use crate::DecodeError;
use crate::DecodeErrorKind;
use crate::vp8l::header::BlockTransformDescriptor;
use crate::vp8l::pixel::extend_rgba_from_argb;
use crate::vp8l::transforms::predictor::PredictorMode;

/// Validated dimensions shared by both predictor storage paths.
struct PredictorLayout {
    width: usize,
    height: usize,
    row_bytes: usize,
    mode_width: usize,
    block_size: usize,
}

fn predictor_layout(
    descriptor: BlockTransformDescriptor,
    mode_pixels: &[u32],
) -> Result<PredictorLayout, DecodeError> {
    let width = usize::try_from(descriptor.image_width).map_err(|_| {
        DecodeError::new(
            DecodeErrorKind::LimitExceeded,
            None,
            "VP8L predictor image width does not fit usize",
        )
    })?;
    let height = usize::try_from(descriptor.image_height).map_err(|_| {
        DecodeError::new(
            DecodeErrorKind::LimitExceeded,
            None,
            "VP8L predictor image height does not fit usize",
        )
    })?;
    let row_bytes = width.checked_mul(4).ok_or_else(|| {
        DecodeError::new(
            DecodeErrorKind::LimitExceeded,
            None,
            "VP8L predictor row byte size overflow",
        )
    })?;
    let mode_width = usize::try_from(descriptor.transform_width).map_err(|_| {
        DecodeError::new(
            DecodeErrorKind::LimitExceeded,
            None,
            "VP8L predictor mode width does not fit usize",
        )
    })?;
    let expected_modes = mode_width
        .checked_mul(usize::try_from(descriptor.transform_height).map_err(|_| {
            DecodeError::new(
                DecodeErrorKind::LimitExceeded,
                None,
                "VP8L predictor mode height does not fit usize",
            )
        })?)
        .ok_or_else(|| {
            DecodeError::new(
                DecodeErrorKind::LimitExceeded,
                None,
                "VP8L predictor mode pixel count overflow",
            )
        })?;
    if mode_pixels.len() != expected_modes {
        return Err(DecodeError::new(
            DecodeErrorKind::InvalidBitstream,
            None,
            "VP8L predictor mode image has unexpected dimensions",
        ));
    }
    let block_size = usize::try_from(descriptor.block_size()).map_err(|_| {
        DecodeError::new(
            DecodeErrorKind::LimitExceeded,
            None,
            "VP8L predictor block size does not fit usize",
        )
    })?;
    Ok(PredictorLayout {
        width,
        height,
        row_bytes,
        mode_width,
        block_size,
    })
}

/// Converts packed residuals one row at a time and reconstructs each row while
/// it is still cache-hot, avoiding a separate full-frame RGBA conversion pass.
pub(in crate::vp8l) fn inverse_predictor_argb_to_rgba(
    pixels: &[u32],
    descriptor: BlockTransformDescriptor,
    mode_pixels: &[u32],
) -> Result<Vec<u8>, DecodeError> {
    let layout = predictor_layout(descriptor, mode_pixels)?;
    let expected_pixels = layout.width.checked_mul(layout.height).ok_or_else(|| {
        DecodeError::new(
            DecodeErrorKind::LimitExceeded,
            None,
            "VP8L predictor image pixel count overflow",
        )
    })?;
    if pixels.len() != expected_pixels {
        return Err(DecodeError::new(
            DecodeErrorKind::InvalidBitstream,
            None,
            "VP8L predictor output length does not match image dimensions",
        ));
    }
    let expected_bytes = layout.row_bytes.checked_mul(layout.height).ok_or_else(|| {
        DecodeError::new(
            DecodeErrorKind::LimitExceeded,
            None,
            "VP8L predictor image byte size overflow",
        )
    })?;
    let mut rgba = Vec::new();
    rgba.try_reserve_exact(expected_bytes).map_err(|_| {
        DecodeError::new(
            DecodeErrorKind::AllocationFailed,
            None,
            "RGBA output allocation failed",
        )
    })?;

    for (y, residual_row) in pixels.chunks_exact(layout.width).enumerate() {
        extend_rgba_from_argb(&mut rgba, residual_row);
        let row_start = y * layout.row_bytes;
        if y == 0 {
            let current = &mut rgba[..layout.row_bytes];
            current[3] = current[3].wrapping_add(255);
            for byte in 4..layout.row_bytes {
                current[byte] = current[byte].wrapping_add(current[byte - 4]);
            }
            continue;
        }

        let (previous_rows, current_row) = rgba.split_at_mut(row_start);
        let top = &previous_rows[row_start - layout.row_bytes..row_start];
        let current = &mut current_row[..layout.row_bytes];
        for channel in 0..4 {
            current[channel] = current[channel].wrapping_add(top[channel]);
        }
        let mode_row = (y / layout.block_size) * layout.mode_width;
        let mut x = 1;
        while x < layout.width {
            let block_x = x / layout.block_size;
            let mode =
                PredictorMode::try_from(((mode_pixels[mode_row + block_x] >> 8) & 0x0f) as u8)
                    .map_err(|_| {
                        DecodeError::new(
                            DecodeErrorKind::InvalidBitstream,
                            None,
                            "VP8L predictor mode must be in 0..=13",
                        )
                    })?;
            let x_end = (x & !(layout.block_size - 1))
                .saturating_add(layout.block_size)
                .min(layout.width);
            apply_predictor_run_rgba(current, top, x, x_end, mode);
            x = x_end;
        }
    }
    Ok(rgba)
}

/// Reconstructs residuals that are already stored in final RGBA byte order.
pub(in crate::vp8l) fn inverse_predictor_rgba(
    pixels: &mut [u8],
    descriptor: BlockTransformDescriptor,
    mode_pixels: &[u32],
) -> Result<(), DecodeError> {
    let layout = predictor_layout(descriptor, mode_pixels)?;
    let expected_bytes = layout.row_bytes.checked_mul(layout.height).ok_or_else(|| {
        DecodeError::new(
            DecodeErrorKind::LimitExceeded,
            None,
            "VP8L predictor image byte size overflow",
        )
    })?;
    if pixels.len() != expected_bytes {
        return Err(DecodeError::new(
            DecodeErrorKind::InvalidBitstream,
            None,
            "VP8L predictor output length does not match image dimensions",
        ));
    }

    // The first row has fixed black/left predictors independent of its mode
    // subimage. RGBA's alpha byte is the only nonzero black component.
    pixels[3] = pixels[3].wrapping_add(255);
    for byte in 4..layout.row_bytes {
        pixels[byte] = pixels[byte].wrapping_add(pixels[byte - 4]);
    }

    for y in 1..layout.height {
        let row_start = y * layout.row_bytes;
        let (previous_rows, current_and_after) = pixels.split_at_mut(row_start);
        let top = &previous_rows[row_start - layout.row_bytes..row_start];
        let current = &mut current_and_after[..layout.row_bytes];
        for channel in 0..4 {
            current[channel] = current[channel].wrapping_add(top[channel]);
        }

        let mode_row = (y / layout.block_size) * layout.mode_width;
        let mut x = 1;
        while x < layout.width {
            let block_x = x / layout.block_size;
            let mode =
                PredictorMode::try_from(((mode_pixels[mode_row + block_x] >> 8) & 0x0f) as u8)
                    .map_err(|_| {
                        DecodeError::new(
                            DecodeErrorKind::InvalidBitstream,
                            None,
                            "VP8L predictor mode must be in 0..=13",
                        )
                    })?;
            let x_end = (x & !(layout.block_size - 1))
                .saturating_add(layout.block_size)
                .min(layout.width);
            apply_predictor_run_rgba(current, top, x, x_end, mode);
            x = x_end;
        }
    }
    Ok(())
}

fn apply_predictor_run_rgba(
    current: &mut [u8],
    top: &[u8],
    start_x: usize,
    end_x: usize,
    mode: PredictorMode,
) {
    macro_rules! reconstruct {
        ($x:ident, $prediction:expr) => {
            for $x in start_x..end_x {
                let prediction = $prediction;
                add_rgba_pixel(current, $x, prediction);
            }
        };
    }

    match mode {
        PredictorMode::OpaqueBlack => {
            for pixel in current[start_x * 4..end_x * 4].chunks_exact_mut(4) {
                pixel[3] = pixel[3].wrapping_add(255);
            }
        }
        PredictorMode::Left => {
            for byte in start_x * 4..end_x * 4 {
                current[byte] = current[byte].wrapping_add(current[byte - 4]);
            }
        }
        PredictorMode::Top => add_aligned_rgba(&mut current[start_x * 4..end_x * 4], top, start_x),
        PredictorMode::TopLeft => {
            add_aligned_rgba(&mut current[start_x * 4..end_x * 4], top, start_x - 1);
        }
        PredictorMode::TopRight => {
            reconstruct!(x, top_right_rgba(current, top, x));
        }
        PredictorMode::AverageLeftTopRightTop => {
            reconstruct!(
                x,
                average_rgba(
                    average_rgba(rgba_pixel(current, x - 1), top_right_rgba(current, top, x)),
                    rgba_pixel(top, x),
                )
            );
        }
        PredictorMode::AverageLeftTopLeft => {
            reconstruct!(
                x,
                average_rgba(rgba_pixel(current, x - 1), rgba_pixel(top, x - 1))
            );
        }
        PredictorMode::AverageLeftTop => {
            reconstruct!(
                x,
                average_rgba(rgba_pixel(current, x - 1), rgba_pixel(top, x))
            );
        }
        PredictorMode::AverageTopLeftTop => {
            reconstruct!(x, average_rgba(rgba_pixel(top, x - 1), rgba_pixel(top, x)));
        }
        PredictorMode::AverageTopTopRight => {
            reconstruct!(
                x,
                average_rgba(rgba_pixel(top, x), top_right_rgba(current, top, x))
            );
        }
        PredictorMode::AverageLeftTopLeftTopTopRight => {
            reconstruct!(
                x,
                average_rgba(
                    average_rgba(rgba_pixel(current, x - 1), rgba_pixel(top, x - 1)),
                    average_rgba(rgba_pixel(top, x), top_right_rgba(current, top, x)),
                )
            );
        }
        PredictorMode::Select => apply_select_rgba(current, top, start_x, end_x),
        PredictorMode::ClampAddSubtractFull => {
            apply_clamped_add_subtract_full_rgba(current, top, start_x, end_x);
        }
        PredictorMode::ClampAddSubtractHalf => {
            reconstruct!(
                x,
                clamp_add_subtract_half_rgba(
                    rgba_pixel(current, x - 1),
                    rgba_pixel(top, x),
                    rgba_pixel(top, x - 1),
                )
            );
        }
    }
}

/// Reconstructs VP8L's select predictor over one mode run.
///
/// Select is the only predictor used by every method-0 CLIC stream. It has a
/// left-to-right dependency, so treating it as four independent byte slices
/// does not expose useful parallelism. Keeping the reconstructed left pixel
/// in a local value instead avoids reloading it and avoids constructing the
/// unused top-right neighbor required by the generic predictor adapter.
fn apply_select_rgba(current: &mut [u8], top: &[u8], start_x: usize, end_x: usize) {
    let byte_len = (end_x - start_x) * 4;
    let (reconstructed, residual_and_after) = current.split_at_mut(start_x * 4);
    let mut left: [u8; 4] = reconstructed[reconstructed.len() - 4..]
        .try_into()
        .expect("predictor run has a reconstructed left pixel");
    let residuals = &mut residual_and_after[..byte_len];
    let top_left = &top[(start_x - 1) * 4..][..byte_len];
    let top = &top[start_x * 4..][..byte_len];

    for ((residual, top_left), top) in residuals
        .chunks_exact_mut(4)
        .zip(top_left.chunks_exact(4))
        .zip(top.chunks_exact(4))
    {
        // For p = left + top - top_left, the distances to left and top
        // simplify to |top - top_left| and |left - top_left| respectively.
        // On a tie VP8L selects top.
        let top_distance = i16::from(top[0]).abs_diff(i16::from(top_left[0]))
            + i16::from(top[1]).abs_diff(i16::from(top_left[1]))
            + i16::from(top[2]).abs_diff(i16::from(top_left[2]))
            + i16::from(top[3]).abs_diff(i16::from(top_left[3]));
        let left_distance = i16::from(left[0]).abs_diff(i16::from(top_left[0]))
            + i16::from(left[1]).abs_diff(i16::from(top_left[1]))
            + i16::from(left[2]).abs_diff(i16::from(top_left[2]))
            + i16::from(left[3]).abs_diff(i16::from(top_left[3]));
        let prediction = if top_distance < left_distance {
            left
        } else {
            [top[0], top[1], top[2], top[3]]
        };
        left = [
            residual[0].wrapping_add(prediction[0]),
            residual[1].wrapping_add(prediction[1]),
            residual[2].wrapping_add(prediction[2]),
            residual[3].wrapping_add(prediction[3]),
        ];
        residual.copy_from_slice(&left);
    }
}

fn add_aligned_rgba(current: &mut [u8], top: &[u8], top_start_x: usize) {
    let top = &top[top_start_x * 4..top_start_x * 4 + current.len()];
    for (residual, &prediction) in current.iter_mut().zip(top) {
        *residual = residual.wrapping_add(prediction);
    }
}

fn apply_clamped_add_subtract_full_rgba(
    current: &mut [u8],
    top: &[u8],
    start_x: usize,
    end_x: usize,
) {
    let byte_len = (end_x - start_x) * 4;
    let (reconstructed, residual_and_after) = current.split_at_mut(start_x * 4);
    let mut left: [u8; 4] = reconstructed[reconstructed.len() - 4..]
        .try_into()
        .expect("predictor run has a reconstructed left pixel");
    let residuals = &mut residual_and_after[..byte_len];
    let top_left = &top[(start_x - 1) * 4..][..byte_len];
    let top = &top[start_x * 4..][..byte_len];

    for ((residual, top_left), top) in residuals
        .chunks_exact_mut(4)
        .zip(top_left.chunks_exact(4))
        .zip(top.chunks_exact(4))
    {
        left = [
            residual[0].wrapping_add(clamp_add_subtract_component(left[0], top[0], top_left[0])),
            residual[1].wrapping_add(clamp_add_subtract_component(left[1], top[1], top_left[1])),
            residual[2].wrapping_add(clamp_add_subtract_component(left[2], top[2], top_left[2])),
            residual[3].wrapping_add(clamp_add_subtract_component(left[3], top[3], top_left[3])),
        ];
        residual.copy_from_slice(&left);
    }
}

#[inline]
fn clamp_add_subtract_component(left: u8, top: u8, top_left: u8) -> u8 {
    (i16::from(left) + i16::from(top) - i16::from(top_left)).clamp(0, 255) as u8
}

#[inline]
fn rgba_pixel(pixels: &[u8], x: usize) -> [u8; 4] {
    let offset = x * 4;
    pixels[offset..offset + 4]
        .try_into()
        .expect("validated RGBA pixel")
}

#[inline]
fn top_right_rgba(current: &[u8], top: &[u8], x: usize) -> [u8; 4] {
    if x + 1 < top.len() / 4 {
        rgba_pixel(top, x + 1)
    } else {
        rgba_pixel(current, 0)
    }
}

#[inline]
fn add_rgba_pixel(pixels: &mut [u8], x: usize, prediction: [u8; 4]) {
    let offset = x * 4;
    for channel in 0..4 {
        pixels[offset + channel] = pixels[offset + channel].wrapping_add(prediction[channel]);
    }
}

#[inline]
fn average_rgba(first: [u8; 4], second: [u8; 4]) -> [u8; 4] {
    [
        (first[0] & second[0]).wrapping_add((first[0] ^ second[0]) >> 1),
        (first[1] & second[1]).wrapping_add((first[1] ^ second[1]) >> 1),
        (first[2] & second[2]).wrapping_add((first[2] ^ second[2]) >> 1),
        (first[3] & second[3]).wrapping_add((first[3] ^ second[3]) >> 1),
    ]
}

#[inline]
fn clamp_add_subtract_half_component(average: u8, top_left: u8) -> u8 {
    let average = i16::from(average);
    (average + (average - i16::from(top_left)) / 2).clamp(0, 255) as u8
}

#[inline]
fn clamp_add_subtract_half_rgba(left: [u8; 4], top: [u8; 4], top_left: [u8; 4]) -> [u8; 4] {
    let average = average_rgba(left, top);
    [
        clamp_add_subtract_half_component(average[0], top_left[0]),
        clamp_add_subtract_half_component(average[1], top_left[1]),
        clamp_add_subtract_half_component(average[2], top_left[2]),
        clamp_add_subtract_half_component(average[3], top_left[3]),
    ]
}

/// Test-only packed reference used to validate the production RGBA predictor.
///
/// Keeping this structurally independent makes the fourteen-mode differential
/// test useful; it is not selected or compiled into the production decoder.
#[cfg(test)]
fn inverse_predictor_argb_reference(
    pixels: &mut [u32],
    descriptor: BlockTransformDescriptor,
    mode_pixels: &[u32],
) -> Result<(), DecodeError> {
    let width = usize::try_from(descriptor.image_width).map_err(|_| {
        DecodeError::new(
            DecodeErrorKind::LimitExceeded,
            None,
            "VP8L predictor image width does not fit usize",
        )
    })?;
    let height = usize::try_from(descriptor.image_height).map_err(|_| {
        DecodeError::new(
            DecodeErrorKind::LimitExceeded,
            None,
            "VP8L predictor image height does not fit usize",
        )
    })?;
    let expected_pixels = width.checked_mul(height).ok_or_else(|| {
        DecodeError::new(
            DecodeErrorKind::LimitExceeded,
            None,
            "VP8L predictor image pixel count overflow",
        )
    })?;
    if pixels.len() != expected_pixels {
        return Err(DecodeError::new(
            DecodeErrorKind::InvalidBitstream,
            None,
            "VP8L predictor output length does not match image dimensions",
        ));
    }
    let mode_width = usize::try_from(descriptor.transform_width).map_err(|_| {
        DecodeError::new(
            DecodeErrorKind::LimitExceeded,
            None,
            "VP8L predictor mode width does not fit usize",
        )
    })?;
    let expected_modes = mode_width
        .checked_mul(usize::try_from(descriptor.transform_height).map_err(|_| {
            DecodeError::new(
                DecodeErrorKind::LimitExceeded,
                None,
                "VP8L predictor mode height does not fit usize",
            )
        })?)
        .ok_or_else(|| {
            DecodeError::new(
                DecodeErrorKind::LimitExceeded,
                None,
                "VP8L predictor mode pixel count overflow",
            )
        })?;
    if mode_pixels.len() != expected_modes {
        return Err(DecodeError::new(
            DecodeErrorKind::InvalidBitstream,
            None,
            "VP8L predictor mode image has unexpected dimensions",
        ));
    }

    let block_size = usize::try_from(descriptor.block_size()).map_err(|_| {
        DecodeError::new(
            DecodeErrorKind::LimitExceeded,
            None,
            "VP8L predictor block size does not fit usize",
        )
    })?;

    // VP8L fixes the top-left predictor to opaque black and the remainder of
    // the first row to the reconstructed pixel on the left.
    pixels[0] = add_argb_pixels(pixels[0], 0xff00_0000);
    for x in 1..width {
        pixels[x] = add_argb_pixels(pixels[x], pixels[x - 1]);
    }

    for y in 1..height {
        let row_start = y * width;
        // The first pixel of every later row always predicts from above.
        pixels[row_start] = add_argb_pixels(pixels[row_start], pixels[row_start - width]);

        let mode_row = (y / block_size) * mode_width;
        let mut x = 1;
        while x < width {
            let block_x = x / block_size;
            let mode =
                PredictorMode::try_from(((mode_pixels[mode_row + block_x] >> 8) & 0x0f) as u8)
                    .map_err(|_| {
                        DecodeError::new(
                            DecodeErrorKind::InvalidBitstream,
                            None,
                            "VP8L predictor mode must be in 0..=13",
                        )
                    })?;
            let x_end = (x & !(block_size - 1))
                .saturating_add(block_size)
                .min(width);
            apply_predictor_run(pixels, row_start + x, row_start + x_end, width, mode);
            x = x_end;
        }
    }
    Ok(())
}

#[cfg(test)]
fn apply_predictor_run(
    pixels: &mut [u32],
    start: usize,
    end: usize,
    width: usize,
    mode: PredictorMode,
) {
    macro_rules! reconstruct {
        ($offset:ident, $prediction:expr) => {
            for $offset in start..end {
                pixels[$offset] = add_argb_pixels(pixels[$offset], $prediction);
            }
        };
    }

    match mode {
        PredictorMode::OpaqueBlack => reconstruct!(offset, 0xff00_0000),
        PredictorMode::Left => reconstruct!(offset, pixels[offset - 1]),
        PredictorMode::Top => reconstruct!(offset, pixels[offset - width]),
        PredictorMode::TopRight => reconstruct!(offset, pixels[offset + 1 - width]),
        PredictorMode::TopLeft => reconstruct!(offset, pixels[offset - 1 - width]),
        PredictorMode::AverageLeftTopRightTop => reconstruct!(
            offset,
            average_argb3(
                pixels[offset - 1],
                pixels[offset + 1 - width],
                pixels[offset - width],
            )
        ),
        PredictorMode::AverageLeftTopLeft => reconstruct!(
            offset,
            average_argb2(pixels[offset - 1], pixels[offset - 1 - width])
        ),
        PredictorMode::AverageLeftTop => reconstruct!(
            offset,
            average_argb2(pixels[offset - 1], pixels[offset - width])
        ),
        PredictorMode::AverageTopLeftTop => reconstruct!(
            offset,
            average_argb2(pixels[offset - 1 - width], pixels[offset - width])
        ),
        PredictorMode::AverageTopTopRight => reconstruct!(
            offset,
            average_argb2(pixels[offset - width], pixels[offset + 1 - width])
        ),
        PredictorMode::AverageLeftTopLeftTopTopRight => reconstruct!(
            offset,
            average_argb4(
                pixels[offset - 1],
                pixels[offset - 1 - width],
                pixels[offset - width],
                pixels[offset + 1 - width],
            )
        ),
        PredictorMode::Select => reconstruct!(
            offset,
            select_argb(
                pixels[offset - 1],
                pixels[offset - width],
                pixels[offset - 1 - width],
            )
        ),
        PredictorMode::ClampAddSubtractFull => reconstruct!(
            offset,
            clamp_add_subtract_full_argb(
                pixels[offset - 1],
                pixels[offset - width],
                pixels[offset - 1 - width],
            )
        ),
        PredictorMode::ClampAddSubtractHalf => reconstruct!(
            offset,
            clamp_add_subtract_half_argb(
                pixels[offset - 1],
                pixels[offset - width],
                pixels[offset - 1 - width],
            )
        ),
    }
}

#[cfg(test)]
#[inline]
fn add_argb_pixels(first: u32, second: u32) -> u32 {
    const ALPHA_GREEN: u32 = 0xff00_ff00;
    const RED_BLUE: u32 = 0x00ff_00ff;
    let alpha_green = (first & ALPHA_GREEN).wrapping_add(second & ALPHA_GREEN);
    let red_blue = (first & RED_BLUE).wrapping_add(second & RED_BLUE);
    (alpha_green & ALPHA_GREEN) | (red_blue & RED_BLUE)
}

#[cfg(test)]
#[inline]
fn average_argb2(first: u32, second: u32) -> u32 {
    (((first ^ second) & 0xfefe_fefe) >> 1).wrapping_add(first & second)
}

#[cfg(test)]
#[inline]
fn average_argb3(first: u32, second: u32, third: u32) -> u32 {
    average_argb2(average_argb2(first, second), third)
}

#[cfg(test)]
#[inline]
fn average_argb4(first: u32, second: u32, third: u32, fourth: u32) -> u32 {
    average_argb2(average_argb2(first, second), average_argb2(third, fourth))
}

#[cfg(test)]
#[inline]
fn select_argb(left: u32, top: u32, top_left: u32) -> u32 {
    let distance_difference = select_component(top >> 24, left >> 24, top_left >> 24)
        + select_component(
            (top >> 16) & 0xff,
            (left >> 16) & 0xff,
            (top_left >> 16) & 0xff,
        )
        + select_component(
            (top >> 8) & 0xff,
            (left >> 8) & 0xff,
            (top_left >> 8) & 0xff,
        )
        + select_component(top & 0xff, left & 0xff, top_left & 0xff);
    if distance_difference <= 0 { top } else { left }
}

#[cfg(test)]
#[inline]
fn select_component(first: u32, second: u32, reference: u32) -> i32 {
    let first = first as i32 - reference as i32;
    let second = second as i32 - reference as i32;
    second.abs() - first.abs()
}

#[cfg(test)]
fn clamp_add_subtract_full_argb(first: u32, second: u32, third: u32) -> u32 {
    pack_argb_components(|shift| {
        component(first, shift) + component(second, shift) - component(third, shift)
    })
}

#[cfg(test)]
fn clamp_add_subtract_half_argb(first: u32, second: u32, third: u32) -> u32 {
    let average = average_argb2(first, second);
    pack_argb_components(|shift| {
        let value = component(average, shift);
        value + (value - component(third, shift)) / 2
    })
}

#[cfg(test)]
#[inline]
fn component(pixel: u32, shift: u32) -> i32 {
    ((pixel >> shift) & 0xff) as i32
}

#[cfg(test)]
fn pack_argb_components(mut value_at: impl FnMut(u32) -> i32) -> u32 {
    let blue = value_at(0).clamp(0, 255) as u32;
    let green = value_at(8).clamp(0, 255) as u32;
    let red = value_at(16).clamp(0, 255) as u32;
    let alpha = value_at(24).clamp(0, 255) as u32;
    (alpha << 24) | (red << 16) | (green << 8) | blue
}

#[cfg(test)]
#[path = "inverse_predictor_tests.rs"]
mod tests;
