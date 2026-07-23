//! Bounded forward predictor planning and residual generation.

use super::source_analysis::forward_color_pixel;
use super::{ColorTransformPlan, EncodeError};

const CANDIDATE_MODES: [u8; 5] = [1, 2, 7, 11, 12];
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
        let block_size = 1_usize << ADAPTIVE_PREDICTOR_BLOCK_BITS;
        let block_width = width.div_ceil(block_size);
        let block_height = height.div_ceil(block_size);
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
                let mut scores = [0_u64; CANDIDATE_MODES.len()];
                for y in y_start..y_end {
                    for x in x_start..x_end {
                        let index = y * width + x;
                        let current =
                            transformed_pixel(rgba, index, subtract_green, color_transform);
                        for (score, &mode) in scores.iter_mut().zip(&CANDIDATE_MODES) {
                            let predictor = predictor_at(
                                rgba,
                                index,
                                width,
                                subtract_green,
                                color_transform,
                                mode,
                            );
                            *score = score.saturating_add(residual_score(current, predictor));
                        }
                    }
                }
                let mode = scores
                    .iter()
                    .enumerate()
                    .min_by_key(|&(index, score)| (*score, index))
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
    match mode {
        1 => left,
        2 => top,
        7 => average(left, top),
        11 => select(left, top, top_left),
        12 => clamp_add_subtract(left, top, top_left),
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

fn residual_score(current: [u8; 4], predictor: [u8; 4]) -> u64 {
    subtract_pixel(current, predictor)
        .into_iter()
        .map(|value| u64::from(value.min(value.wrapping_neg())))
        .sum()
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

#[cfg(test)]
#[path = "predictor_plan_tests.rs"]
mod tests;
