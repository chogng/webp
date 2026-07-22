//! Scalar, reconstruction-aware RGB-to-YUV420 sampling for VP8 encoding.
//!
//! This owner ports the 8-bit sRGB/WebP-matrix portion of upstream SharpYUV.
//! It owns only the iterative chroma reconstruction problem. `yuv_image`
//! owns VP8 plane allocation and macroblock padding, while the frame writer
//! owns prediction, quantization, and bitstream emission.

use crate::vp8::Vp8EncodeError;

use self::gamma::gamma_to_linear;
use self::gamma::linear_to_gamma;

mod gamma;

const CHANNELS: usize = 3;
const ITERATIONS: usize = 4;
const INPUT_PRECISION: i32 = 2;
const WORKING_BIT_DEPTH: u32 = 10;
const WORKING_MAX: i32 = (1 << WORKING_BIT_DEPTH) - 1;
const YUV_FIX: i32 = 16;
const WEBP_Y: [i32; 4] = [16_839, 33_059, 6_420, 16 << YUV_FIX];
const WEBP_U: [i32; 4] = [-9_719, -19_081, 28_800, 128 << YUV_FIX];
const WEBP_V: [i32; 4] = [28_800, -24_116, -4_684, 128 << YUV_FIX];

pub(super) struct SharpYuvPlanes<'a> {
    pub(super) y_stride: usize,
    pub(super) uv_stride: usize,
    pub(super) y: &'a mut [u8],
    pub(super) u: &'a mut [u8],
    pub(super) v: &'a mut [u8],
}

/// Fills macroblock-padded VP8 planes with the scalar SharpYUV 4:2:0 result.
pub(super) fn convert_rgba_to_yuv420(
    width: u32,
    height: u32,
    rgba: &[u8],
    planes: SharpYuvPlanes<'_>,
) -> Result<(), Vp8EncodeError> {
    let SharpYuvPlanes {
        y_stride,
        uv_stride,
        y,
        u,
        v,
    } = planes;
    let width = usize::try_from(width).map_err(|_| Vp8EncodeError::InvalidDimensions)?;
    let height = usize::try_from(height).map_err(|_| Vp8EncodeError::InvalidDimensions)?;
    let padded_width = width
        .checked_add(1)
        .ok_or(Vp8EncodeError::AllocationFailed)?
        & !1;
    let padded_height = height
        .checked_add(1)
        .ok_or(Vp8EncodeError::AllocationFailed)?
        & !1;
    let uv_width = padded_width / 2;
    let uv_height = padded_height / 2;
    if y_stride < width
        || uv_stride < uv_width
        || y.len() < y_stride.saturating_mul(height)
        || u.len() < uv_stride.saturating_mul(uv_height)
        || v.len() < uv_stride.saturating_mul(uv_height)
    {
        return Err(Vp8EncodeError::InvalidPlaneLayout);
    }

    let mut state = State::new(padded_width, padded_height)?;
    state.import_rgba(width, height, rgba)?;
    state.refine()?;
    let visible = state.convert(width, height)?;
    copy_with_edge_padding(&visible.y, width, height, y, y_stride);
    copy_with_edge_padding(&visible.u, uv_width, uv_height, u, uv_stride);
    copy_with_edge_padding(&visible.v, uv_width, uv_height, v, uv_stride);
    Ok(())
}

struct State {
    width: usize,
    height: usize,
    uv_width: usize,
    best_y: Vec<u16>,
    target_y: Vec<u16>,
    best_uv: Vec<i16>,
    target_uv: Vec<i16>,
}

impl State {
    fn new(width: usize, height: usize) -> Result<Self, Vp8EncodeError> {
        let pixels = width
            .checked_mul(height)
            .ok_or(Vp8EncodeError::AllocationFailed)?;
        let uv = pixels
            .checked_div(4)
            .and_then(|count| count.checked_mul(CHANNELS))
            .ok_or(Vp8EncodeError::AllocationFailed)?;
        Ok(Self {
            width,
            height,
            uv_width: width / 2,
            best_y: zeroed(pixels)?,
            target_y: zeroed(pixels)?,
            best_uv: zeroed(uv)?,
            target_uv: zeroed(uv)?,
        })
    }

    fn import_rgba(
        &mut self,
        source_width: usize,
        source_height: usize,
        rgba: &[u8],
    ) -> Result<(), Vp8EncodeError> {
        let mut source_first = zeroed(self.width * CHANNELS)?;
        let mut source_second = zeroed(self.width * CHANNELS)?;
        for row in (0..self.height).step_by(2) {
            import_row(
                rgba,
                source_width,
                source_height,
                row.min(source_height - 1),
                self.width,
                &mut source_first,
            );
            import_row(
                rgba,
                source_width,
                source_height,
                (row + 1).min(source_height - 1),
                self.width,
                &mut source_second,
            );
            let y_offset = row * self.width;
            store_gray(
                &source_first,
                &mut self.best_y[y_offset..y_offset + self.width],
            );
            store_gray(
                &source_second,
                &mut self.best_y[y_offset + self.width..y_offset + 2 * self.width],
            );
            update_w(
                &source_first,
                &mut self.target_y[y_offset..y_offset + self.width],
            );
            update_w(
                &source_second,
                &mut self.target_y[y_offset + self.width..y_offset + 2 * self.width],
            );
            let uv_offset = (row / 2) * self.uv_width * CHANNELS;
            update_chroma(
                &source_first,
                &source_second,
                self.uv_width,
                &mut self.target_uv[uv_offset..uv_offset + self.uv_width * CHANNELS],
            );
            self.best_uv[uv_offset..uv_offset + self.uv_width * CHANNELS]
                .copy_from_slice(&self.target_uv[uv_offset..uv_offset + self.uv_width * CHANNELS]);
        }
        Ok(())
    }

    fn refine(&mut self) -> Result<(), Vp8EncodeError> {
        let mut previous_difference = u64::MAX;
        let threshold = u64::try_from(self.width * self.height * 3).unwrap_or(u64::MAX);
        let mut source_first = zeroed(self.width * CHANNELS)?;
        let mut source_second = zeroed(self.width * CHANNELS)?;
        let mut rgb_first = zeroed(self.width)?;
        let mut rgb_second = zeroed(self.width)?;
        let mut rgb_uv = zeroed(self.uv_width * CHANNELS)?;
        let uv_row_len = self.uv_width * CHANNELS;

        for iteration in 0..ITERATIONS {
            let mut difference = 0_u64;
            for row in (0..self.height).step_by(2) {
                let uv_row = row / 2;
                let previous_start = uv_row.saturating_sub(1) * uv_row_len;
                let current_start = uv_row * uv_row_len;
                let next_start = (uv_row + 1).min(self.height / 2 - 1) * uv_row_len;
                interpolate_rows(
                    &self.best_y[row * self.width..(row + 2) * self.width],
                    self.width,
                    self.uv_width,
                    ChromaRows {
                        previous: &self.best_uv[previous_start..previous_start + uv_row_len],
                        current: &self.best_uv[current_start..current_start + uv_row_len],
                        next: &self.best_uv[next_start..next_start + uv_row_len],
                    },
                    RgbRowsMut {
                        first: &mut source_first,
                        second: &mut source_second,
                    },
                );
                update_w(&source_first, &mut rgb_first);
                update_w(&source_second, &mut rgb_second);
                update_chroma(&source_first, &source_second, self.uv_width, &mut rgb_uv);
                let y_offset = row * self.width;
                difference += update_y(
                    &self.target_y[y_offset..y_offset + self.width],
                    &rgb_first,
                    &mut self.best_y[y_offset..y_offset + self.width],
                );
                difference += update_y(
                    &self.target_y[y_offset + self.width..y_offset + 2 * self.width],
                    &rgb_second,
                    &mut self.best_y[y_offset + self.width..y_offset + 2 * self.width],
                );
                update_rgb(
                    &self.target_uv[current_start..current_start + uv_row_len],
                    &rgb_uv,
                    &mut self.best_uv[current_start..current_start + uv_row_len],
                );
            }
            if iteration > 0 && (difference < threshold || difference > previous_difference) {
                break;
            }
            previous_difference = difference;
        }
        Ok(())
    }

    fn convert(&self, width: usize, height: usize) -> Result<VisibleYuv, Vp8EncodeError> {
        let uv_height = height.div_ceil(2);
        let uv_width = width.div_ceil(2);
        let mut y = zeroed(width * height)?;
        let mut u = zeroed(uv_width * uv_height)?;
        let mut v = zeroed(uv_width * uv_height)?;
        for row in 0..height {
            let y_row = &self.best_y[row * self.width..(row + 1) * self.width];
            let uv_row = self.uv_row(row / 2);
            for column in 0..width {
                let offset = column / 2;
                let red = i32::from(uv_row[offset]) + i32::from(y_row[column]);
                let green = i32::from(uv_row[self.uv_width + offset]) + i32::from(y_row[column]);
                let blue = i32::from(uv_row[2 * self.uv_width + offset]) + i32::from(y_row[column]);
                y[row * width + column] = component(red, green, blue, WEBP_Y);
            }
        }
        for row in 0..uv_height {
            let uv_row = self.uv_row(row);
            for column in 0..uv_width {
                let red = i32::from(uv_row[column]);
                let green = i32::from(uv_row[self.uv_width + column]);
                let blue = i32::from(uv_row[2 * self.uv_width + column]);
                u[row * uv_width + column] = component(red, green, blue, WEBP_U);
                v[row * uv_width + column] = component(red, green, blue, WEBP_V);
            }
        }
        Ok(VisibleYuv { y, u, v })
    }

    fn uv_row(&self, row: usize) -> &[i16] {
        let start = row * self.uv_width * CHANNELS;
        &self.best_uv[start..start + self.uv_width * CHANNELS]
    }
}

struct VisibleYuv {
    y: Vec<u8>,
    u: Vec<u8>,
    v: Vec<u8>,
}

fn zeroed<T: Clone + Default>(len: usize) -> Result<Vec<T>, Vp8EncodeError> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(len)
        .map_err(|_| Vp8EncodeError::AllocationFailed)?;
    values.resize(len, T::default());
    Ok(values)
}

fn import_row(
    rgba: &[u8],
    source_width: usize,
    source_height: usize,
    row: usize,
    output_width: usize,
    output: &mut [u16],
) {
    let row = row.min(source_height - 1);
    for destination_column in 0..output_width {
        let source_column = destination_column.min(source_width - 1);
        let offset = (row * source_width + source_column) * 4;
        for channel in 0..CHANNELS {
            output[channel * output_width + destination_column] =
                u16::from(rgba[offset + channel]) << INPUT_PRECISION;
        }
    }
}

fn store_gray(rgb: &[u16], output: &mut [u16]) {
    let width = output.len();
    for index in 0..width {
        output[index] = rgb_to_gray(
            u32::from(rgb[index]),
            u32::from(rgb[width + index]),
            u32::from(rgb[2 * width + index]),
        );
    }
}

fn update_w(rgb: &[u16], output: &mut [u16]) {
    let width = output.len();
    for index in 0..width {
        let red = gamma_to_linear(rgb[index]);
        let green = gamma_to_linear(rgb[width + index]);
        let blue = gamma_to_linear(rgb[2 * width + index]);
        output[index] = linear_to_gamma(u32::from(rgb_to_gray(red, green, blue)));
    }
}

fn update_chroma(first: &[u16], second: &[u16], uv_width: usize, output: &mut [i16]) {
    let width = uv_width * 2;
    for index in 0..uv_width {
        let mut values = [0_i32; CHANNELS];
        for (channel, value) in values.iter_mut().enumerate() {
            let offset = channel * width + 2 * index;
            *value = scale_down(
                first[offset],
                first[offset + 1],
                second[offset],
                second[offset + 1],
            );
        }
        let gray = i32::from(rgb_to_gray(
            values[0] as u32,
            values[1] as u32,
            values[2] as u32,
        ));
        for channel in 0..CHANNELS {
            output[channel * uv_width + index] = (values[channel] - gray) as i16;
        }
    }
}

fn interpolate_rows(
    best_y: &[u16],
    width: usize,
    uv_width: usize,
    chroma: ChromaRows<'_>,
    output: RgbRowsMut<'_>,
) {
    let ChromaRows {
        previous,
        current,
        next,
    } = chroma;
    let RgbRowsMut { first, second } = output;
    for channel in 0..CHANNELS {
        let offset = channel * uv_width;
        let output = channel * width;
        first[output] = filter2(current[offset], previous[offset], best_y[0]);
        second[output] = filter2(current[offset], next[offset], best_y[width]);
        for index in 0..uv_width - 1 {
            let a0 = i32::from(current[offset + index]);
            let a1 = i32::from(current[offset + index + 1]);
            let b0 = i32::from(previous[offset + index]);
            let b1 = i32::from(previous[offset + index + 1]);
            let c0 = i32::from(next[offset + index]);
            let c1 = i32::from(next[offset + index + 1]);
            first[output + 2 * index + 1] = clip_working(
                i32::from(best_y[2 * index + 1]) + ((a0 * 9 + a1 * 3 + b0 * 3 + b1 + 8) >> 4),
            );
            first[output + 2 * index + 2] = clip_working(
                i32::from(best_y[2 * index + 2]) + ((a1 * 9 + a0 * 3 + b1 * 3 + b0 + 8) >> 4),
            );
            second[output + 2 * index + 1] = clip_working(
                i32::from(best_y[width + 2 * index + 1])
                    + ((a0 * 9 + a1 * 3 + c0 * 3 + c1 + 8) >> 4),
            );
            second[output + 2 * index + 2] = clip_working(
                i32::from(best_y[width + 2 * index + 2])
                    + ((a1 * 9 + a0 * 3 + c1 * 3 + c0 + 8) >> 4),
            );
        }
        let last = width - 1;
        let uv_last = uv_width - 1;
        first[output + last] = filter2(
            current[offset + uv_last],
            previous[offset + uv_last],
            best_y[last],
        );
        second[output + last] = filter2(
            current[offset + uv_last],
            next[offset + uv_last],
            best_y[width + last],
        );
    }
}

struct ChromaRows<'a> {
    previous: &'a [i16],
    current: &'a [i16],
    next: &'a [i16],
}

struct RgbRowsMut<'a> {
    first: &'a mut [u16],
    second: &'a mut [u16],
}

fn update_y(target: &[u16], reconstructed: &[u16], best: &mut [u16]) -> u64 {
    target
        .iter()
        .zip(reconstructed)
        .zip(best)
        .map(|((&target, &reconstructed), best)| {
            let difference = i32::from(target) - i32::from(reconstructed);
            *best = clip_working(i32::from(*best) + difference);
            difference.unsigned_abs() as u64
        })
        .sum()
}

fn update_rgb(target: &[i16], reconstructed: &[i16], best: &mut [i16]) {
    for ((&target, &reconstructed), best) in target.iter().zip(reconstructed).zip(best) {
        *best += target - reconstructed;
    }
}

fn copy_with_edge_padding(
    source: &[u8],
    width: usize,
    height: usize,
    destination: &mut [u8],
    stride: usize,
) {
    for row in 0..destination.len() / stride {
        let source_row = row.min(height - 1);
        for column in 0..stride {
            destination[row * stride + column] = source[source_row * width + column.min(width - 1)];
        }
    }
}

fn component(red: i32, green: i32, blue: i32, coefficients: [i32; 4]) -> u8 {
    let rounding = 1_i64 << (YUV_FIX + INPUT_PRECISION - 1);
    let value = i64::from(coefficients[0]) * i64::from(red)
        + i64::from(coefficients[1]) * i64::from(green)
        + i64::from(coefficients[2]) * i64::from(blue)
        + (i64::from(coefficients[3]) << INPUT_PRECISION)
        + rounding;
    value
        .checked_shr((YUV_FIX + INPUT_PRECISION) as u32)
        .unwrap_or_default()
        .clamp(0, i64::from(u8::MAX)) as u8
}

fn rgb_to_gray(red: u32, green: u32, blue: u32) -> u16 {
    ((13_933 * red + 46_871 * green + 4_732 * blue + (1 << 15)) >> 16) as u16
}

fn scale_down(a: u16, b: u16, c: u16, d: u16) -> i32 {
    let average =
        (gamma_to_linear(a) + gamma_to_linear(b) + gamma_to_linear(c) + gamma_to_linear(d) + 2) / 4;
    i32::from(linear_to_gamma(average))
}

fn filter2(a: i16, b: i16, y: u16) -> u16 {
    clip_working(((i32::from(a) * 3 + i32::from(b) + 2) >> 2) + i32::from(y))
}

fn clip_working(value: i32) -> u16 {
    value.clamp(0, WORKING_MAX) as u16
}

#[cfg(test)]
#[path = "sharp_yuv_tests.rs"]
mod tests;
