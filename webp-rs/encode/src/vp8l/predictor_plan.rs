//! Bounded forward predictor planning and residual generation.

use super::source_analysis::forward_color_pixel;
use super::{ColorTransformPlan, EncodeError};

const CORE_CANDIDATE_MODES: usize = 6;
const EXTENDED_MODE_PIXEL_LIMIT: usize = 1 << 20;
const CANDIDATE_MODES: [u8; 14] = [1, 2, 7, 11, 12, 13, 0, 3, 4, 5, 6, 8, 9, 10];
pub(super) const ADAPTIVE_PREDICTOR_BLOCK_BITS: u8 = 4;

#[derive(Clone)]
pub(super) enum PredictorPlan {
    None,
    Constant(u8),
    Blocks {
        block_bits: u8,
        block_width: usize,
        modes: Vec<u8>,
    },
}

impl PredictorPlan {
    pub(super) fn adaptive(
        rgba: &[u8],
        width: usize,
        subtract_green: bool,
        color_transform: Option<ColorTransformPlan>,
    ) -> Result<Self, EncodeError> {
        let pixels = rgba.len() / 4;
        let height = pixels / width;
        // Keep the full VP8L mode portfolio on ordinary images, but cap the
        // per-pixel work of very large inputs to the six strongest modes.
        let candidate_modes = if pixels <= EXTENDED_MODE_PIXEL_LIMIT {
            &CANDIDATE_MODES[..]
        } else {
            &CANDIDATE_MODES[..CORE_CANDIDATE_MODES]
        };
        let block_size = 1_usize << ADAPTIVE_PREDICTOR_BLOCK_BITS;
        let block_width = width.div_ceil(block_size);
        let block_height = height.div_ceil(block_size);
        let mut transformed = Vec::new();
        transformed
            .try_reserve_exact(pixels)
            .map_err(|_| EncodeError::allocation_failed())?;
        for index in 0..pixels {
            transformed.push(transformed_pixel(
                rgba,
                index,
                subtract_green,
                color_transform,
            ));
        }
        // For a fixed block size, minimizing Shannon entropy is equivalent to
        // maximizing sum(count * log2(count)) across the channel histograms.
        let count_log_count = std::array::from_fn::<_, 257, _>(|count| {
            if count == 0 {
                0
            } else {
                ((count as f64) * (count as f64).log2() * 1024.0).round() as u64
            }
        });
        let mut modes = Vec::new();
        modes
            .try_reserve_exact(
                block_width
                    .checked_mul(block_height)
                    .ok_or_else(EncodeError::output_size_overflow)?,
            )
            .map_err(|_| EncodeError::allocation_failed())?;
        for block_y in 0..block_height {
            for block_x in 0..block_width {
                let x_start = block_x * block_size;
                let y_start = block_y * block_size;
                let x_end = (x_start + block_size).min(width);
                let y_end = (y_start + block_size).min(height);
                let mut concentration = [0_u64; CANDIDATE_MODES.len()];
                let mut histograms = [[[0_u16; 256]; 4]; CANDIDATE_MODES.len()];
                for y in y_start..y_end {
                    for x in x_start..x_end {
                        let index = y * width + x;
                        let current = transformed[index];
                        for (candidate, &mode) in candidate_modes.iter().enumerate() {
                            let predictor =
                                predictor_at_transformed(&transformed, index, width, mode);
                            add_residual(
                                &mut histograms[candidate],
                                &mut concentration[candidate],
                                subtract_pixel(current, predictor),
                                &count_log_count,
                            );
                        }
                    }
                }
                let mode = concentration
                    .get(..candidate_modes.len())
                    .expect("candidate histogram covers active modes")
                    .iter()
                    .enumerate()
                    .max_by_key(|&(index, score)| (*score, usize::MAX - index))
                    .map(|(index, _)| CANDIDATE_MODES[index])
                    .expect("predictor candidate set is nonempty");
                modes.push(mode);
            }
        }
        Ok(Self::Blocks {
            block_bits: ADAPTIVE_PREDICTOR_BLOCK_BITS,
            block_width,
            modes,
        })
    }

    pub(super) const fn is_present(&self) -> bool {
        !matches!(self, Self::None)
    }

    pub(super) const fn block_bits(&self) -> u8 {
        match self {
            Self::None | Self::Constant(_) => 2,
            Self::Blocks { block_bits, .. } => *block_bits,
        }
    }

    pub(super) fn mode_at(&self, index: usize, width: usize) -> Option<u8> {
        match self {
            Self::None => None,
            Self::Constant(mode) => Some(*mode),
            Self::Blocks {
                block_bits,
                block_width,
                modes,
            } => {
                let block_size = 1_usize << block_bits;
                let block =
                    (index / width / block_size) * block_width + (index % width / block_size);
                modes.get(block).copied()
            }
        }
    }
}

fn add_residual(
    histogram: &mut [[u16; 256]; 4],
    concentration: &mut u64,
    residual: [u8; 4],
    count_log_count: &[u64; 257],
) {
    for (channel, symbol) in residual.into_iter().enumerate() {
        let count = &mut histogram[channel][usize::from(symbol)];
        let previous = usize::from(*count);
        *count = count.saturating_add(1);
        *concentration = concentration.saturating_add(
            count_log_count[previous + 1].saturating_sub(count_log_count[previous]),
        );
    }
}

pub(super) fn residual_pixel(
    rgba: &[u8],
    index: usize,
    width: usize,
    subtract_green: bool,
    color_transform: Option<ColorTransformPlan>,
    predictor: &PredictorPlan,
) -> [u8; 4] {
    let current = transformed_pixel(rgba, index, subtract_green, color_transform);
    let predicted = predictor.mode_at(index, width).map_or([0; 4], |mode| {
        predictor_at(rgba, index, width, subtract_green, color_transform, mode)
    });
    subtract_pixel(current, predicted)
}

fn predictor_at(
    rgba: &[u8],
    index: usize,
    width: usize,
    subtract_green: bool,
    color_transform: Option<ColorTransformPlan>,
    mode: u8,
) -> [u8; 4] {
    if index == 0 {
        return [0, 0, 0, u8::MAX];
    }
    let x = index % width;
    if index < width {
        return transformed_pixel(rgba, index - 1, subtract_green, color_transform);
    }
    if x == 0 {
        return transformed_pixel(rgba, index - width, subtract_green, color_transform);
    }
    let left = transformed_pixel(rgba, index - 1, subtract_green, color_transform);
    let top = transformed_pixel(rgba, index - width, subtract_green, color_transform);
    let top_left = transformed_pixel(rgba, index - width - 1, subtract_green, color_transform);
    let top_right = transformed_pixel(
        rgba,
        if x + 1 == width {
            index - x
        } else {
            index - width + 1
        },
        subtract_green,
        color_transform,
    );
    predictor_from_neighbors(mode, left, top, top_left, top_right)
}

fn predictor_at_transformed(pixels: &[[u8; 4]], index: usize, width: usize, mode: u8) -> [u8; 4] {
    if index == 0 {
        return [0, 0, 0, u8::MAX];
    }
    let x = index % width;
    if index < width {
        return pixels[index - 1];
    }
    if x == 0 {
        return pixels[index - width];
    }
    let left = pixels[index - 1];
    let top = pixels[index - width];
    let top_left = pixels[index - width - 1];
    let top_right = pixels[if x + 1 == width {
        index - x
    } else {
        index - width + 1
    }];
    predictor_from_neighbors(mode, left, top, top_left, top_right)
}

fn predictor_from_neighbors(
    mode: u8,
    left: [u8; 4],
    top: [u8; 4],
    top_left: [u8; 4],
    top_right: [u8; 4],
) -> [u8; 4] {
    match mode {
        0 => [0, 0, 0, u8::MAX],
        1 => left,
        2 => top,
        3 => top_right,
        4 => top_left,
        5 => average(average(left, top_right), top),
        6 => average(left, top_left),
        7 => average(left, top),
        8 => average(top_left, top),
        9 => average(top, top_right),
        10 => average(average(left, top_left), average(top, top_right)),
        11 => select(left, top, top_left),
        12 => clamp_add_subtract(left, top, top_left),
        13 => clamp_add_subtract_half(left, top, top_left),
        _ => [0, 0, 0, u8::MAX],
    }
}

fn transformed_pixel(
    rgba: &[u8],
    index: usize,
    subtract_green: bool,
    color_transform: Option<ColorTransformPlan>,
) -> [u8; 4] {
    let offset = index * 4;
    let source = [
        rgba[offset],
        rgba[offset + 1],
        rgba[offset + 2],
        rgba[offset + 3],
    ];
    let [red, green, blue, alpha] =
        color_transform.map_or(source, |plan| forward_color_pixel(&source, plan));
    if subtract_green {
        [
            red.wrapping_sub(green),
            green,
            blue.wrapping_sub(green),
            alpha,
        ]
    } else {
        [red, green, blue, alpha]
    }
}

fn subtract_pixel(current: [u8; 4], predictor: [u8; 4]) -> [u8; 4] {
    [
        current[0].wrapping_sub(predictor[0]),
        current[1].wrapping_sub(predictor[1]),
        current[2].wrapping_sub(predictor[2]),
        current[3].wrapping_sub(predictor[3]),
    ]
}

fn average(left: [u8; 4], right: [u8; 4]) -> [u8; 4] {
    let mut output = [0; 4];
    for channel in 0..4 {
        output[channel] = ((u16::from(left[channel]) + u16::from(right[channel])) / 2) as u8;
    }
    output
}

fn select(left: [u8; 4], top: [u8; 4], top_left: [u8; 4]) -> [u8; 4] {
    let top_distance = top
        .into_iter()
        .zip(top_left)
        .map(|(value, corner)| value.abs_diff(corner) as u16)
        .sum::<u16>();
    let left_distance = left
        .into_iter()
        .zip(top_left)
        .map(|(value, corner)| value.abs_diff(corner) as u16)
        .sum::<u16>();
    if top_distance < left_distance {
        left
    } else {
        top
    }
}

fn clamp_add_subtract(left: [u8; 4], top: [u8; 4], top_left: [u8; 4]) -> [u8; 4] {
    let mut output = [0; 4];
    for channel in 0..4 {
        output[channel] = (i16::from(left[channel]) + i16::from(top[channel])
            - i16::from(top_left[channel]))
        .clamp(0, 255) as u8;
    }
    output
}

fn clamp_add_subtract_half(left: [u8; 4], top: [u8; 4], top_left: [u8; 4]) -> [u8; 4] {
    let averaged = average(left, top);
    let mut output = [0; 4];
    for channel in 0..4 {
        let value = i16::from(averaged[channel]);
        output[channel] = (value + (value - i16::from(top_left[channel])) / 2).clamp(0, 255) as u8;
    }
    output
}

#[cfg(test)]
#[path = "predictor_plan_tests.rs"]
mod tests;
