//! Source-owned VP8L facts used by transform and entropy planning.

use super::ColorTransformPlan;
use super::EncodeError;

const MAX_ENCODER_PALETTE_SIZE: usize = 16;
const MIN_COLOR_TRANSFORM_PIXELS: usize = 256;
const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

const COLOR_TRANSFORM_CANDIDATES: [ColorTransformPlan; 6] = [
    ColorTransformPlan {
        green_to_red: 32,
        green_to_blue: 32,
        red_to_blue: 0,
    },
    ColorTransformPlan {
        green_to_red: 32,
        green_to_blue: 0,
        red_to_blue: 32,
    },
    ColorTransformPlan {
        green_to_red: 0,
        green_to_blue: 32,
        red_to_blue: 32,
    },
    ColorTransformPlan {
        green_to_red: 48,
        green_to_blue: 48,
        red_to_blue: 0,
    },
    ColorTransformPlan {
        green_to_red: -32,
        green_to_blue: -32,
        red_to_blue: 0,
    },
    ColorTransformPlan {
        green_to_red: 64,
        green_to_blue: 64,
        red_to_blue: 0,
    },
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct InputIdentity {
    rgba_bytes: usize,
    fnv1a64: u64,
}

impl InputIdentity {
    pub(super) const fn rgba_bytes(self) -> usize {
        self.rgba_bytes
    }

    #[cfg(test)]
    pub(super) const fn fnv1a64(self) -> u64 {
        self.fnv1a64
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct SourceFacts {
    width: usize,
    height: usize,
    pixels: usize,
    identity: InputIdentity,
    non_opaque_pixels: usize,
    transparent_pixels: usize,
    palette_colors: Option<usize>,
}

impl SourceFacts {
    pub(super) const fn width(self) -> usize {
        self.width
    }

    pub(super) const fn height(self) -> usize {
        self.height
    }

    pub(super) const fn pixels(self) -> usize {
        self.pixels
    }

    pub(super) const fn identity(self) -> InputIdentity {
        self.identity
    }

    pub(super) const fn has_alpha(self) -> bool {
        self.non_opaque_pixels != 0
    }

    #[cfg(test)]
    pub(super) const fn non_opaque_pixels(self) -> usize {
        self.non_opaque_pixels
    }

    #[cfg(test)]
    pub(super) const fn transparent_pixels(self) -> usize {
        self.transparent_pixels
    }

    #[cfg(test)]
    pub(super) const fn palette_colors(self) -> Option<usize> {
        self.palette_colors
    }
}

pub(crate) struct PalettePlan {
    entries: Vec<[u8; 4]>,
    indexed_rgba: Vec<u8>,
    indexed_width: usize,
}

impl PalettePlan {
    pub(super) fn entries(&self) -> &[[u8; 4]] {
        &self.entries
    }

    pub(super) fn indexed_rgba(&self) -> &[u8] {
        &self.indexed_rgba
    }

    pub(super) const fn indexed_width(&self) -> usize {
        self.indexed_width
    }
}

pub(super) struct SourceAnalysis {
    facts: SourceFacts,
    palette: Option<PalettePlan>,
    color_transform: Option<ColorTransformPlan>,
}

impl SourceAnalysis {
    pub(super) fn collect(rgba: &[u8], width: usize) -> Result<Self, EncodeError> {
        let pixels = rgba.len() / 4;
        if width == 0
            || pixels == 0
            || !rgba.len().is_multiple_of(4)
            || !pixels.is_multiple_of(width)
        {
            return Err(EncodeError::output_size_overflow());
        }

        let mut entries = Vec::new();
        entries
            .try_reserve_exact(MAX_ENCODER_PALETTE_SIZE)
            .map_err(|_| EncodeError::allocation_failed())?;
        let mut indices = Vec::new();
        indices
            .try_reserve_exact(pixels)
            .map_err(|_| EncodeError::allocation_failed())?;
        let mut palette_possible = true;
        let mut non_opaque_pixels = 0_usize;
        let mut transparent_pixels = 0_usize;
        let mut identity = FNV_OFFSET_BASIS;
        let mut color_scores = [0_u64; 1 + COLOR_TRANSFORM_CANDIDATES.len()];

        for pixel in rgba.chunks_exact(4) {
            for &byte in pixel {
                identity ^= u64::from(byte);
                identity = identity.wrapping_mul(FNV_PRIME);
            }
            non_opaque_pixels += usize::from(pixel[3] != u8::MAX);
            transparent_pixels += usize::from(pixel[3] == 0);

            color_scores[0] += color_score(pixel[0], pixel[2]);
            for (score, plan) in color_scores[1..].iter_mut().zip(COLOR_TRANSFORM_CANDIDATES) {
                let transformed = forward_color_pixel(pixel, plan);
                *score += color_score(transformed[0], transformed[2]);
            }

            if palette_possible {
                let color = [pixel[0], pixel[1], pixel[2], pixel[3]];
                let index = match entries.iter().position(|entry| *entry == color) {
                    Some(index) => index,
                    None if entries.len() < MAX_ENCODER_PALETTE_SIZE => {
                        entries.push(color);
                        entries.len() - 1
                    }
                    None => {
                        palette_possible = false;
                        indices.clear();
                        continue;
                    }
                };
                indices.push(u8::try_from(index).expect("bounded palette index fits u8"));
            }
        }

        let palette_colors = palette_possible.then_some(entries.len());
        let palette = if palette_possible && indices.len() >= 2 {
            Some(build_palette_plan(entries, indices, width)?)
        } else {
            None
        };
        let color_transform = select_color_transform_from_scores(pixels, color_scores);
        Ok(Self {
            facts: SourceFacts {
                width,
                height: pixels / width,
                pixels,
                identity: InputIdentity {
                    rgba_bytes: rgba.len(),
                    fnv1a64: identity,
                },
                non_opaque_pixels,
                transparent_pixels,
                palette_colors,
            },
            palette,
            color_transform,
        })
    }

    pub(super) const fn facts(&self) -> SourceFacts {
        self.facts
    }

    pub(super) const fn color_transform(&self) -> Option<ColorTransformPlan> {
        self.color_transform
    }

    pub(super) fn into_palette(self) -> Option<PalettePlan> {
        self.palette
    }
}

#[cfg(test)]
pub(super) fn select_color_transform(rgba: &[u8]) -> Option<ColorTransformPlan> {
    let pixels = rgba.len() / 4;
    if pixels < MIN_COLOR_TRANSFORM_PIXELS {
        return None;
    }
    let mut color_scores = [0_u64; 1 + COLOR_TRANSFORM_CANDIDATES.len()];
    for pixel in rgba.chunks_exact(4) {
        color_scores[0] += color_score(pixel[0], pixel[2]);
        for (score, plan) in color_scores[1..].iter_mut().zip(COLOR_TRANSFORM_CANDIDATES) {
            let transformed = forward_color_pixel(pixel, plan);
            *score += color_score(transformed[0], transformed[2]);
        }
    }
    select_color_transform_from_scores(pixels, color_scores)
}

pub(super) fn forward_color_pixel(pixel: &[u8], plan: ColorTransformPlan) -> [u8; 4] {
    let red_delta = color_transform_delta(pixel[1], plan.green_to_red);
    let blue_delta = color_transform_delta(pixel[1], plan.green_to_blue)
        + color_transform_delta(pixel[0], plan.red_to_blue);
    [
        pixel[0].wrapping_sub(red_delta as u8),
        pixel[1],
        pixel[2].wrapping_sub(blue_delta as u8),
        pixel[3],
    ]
}

fn select_color_transform_from_scores(
    pixels: usize,
    scores: [u64; 1 + COLOR_TRANSFORM_CANDIDATES.len()],
) -> Option<ColorTransformPlan> {
    if pixels < MIN_COLOR_TRANSFORM_PIXELS {
        return None;
    }
    let baseline = scores[0];
    let mut selected = None;
    let mut best = baseline;
    for (candidate, score) in COLOR_TRANSFORM_CANDIDATES
        .into_iter()
        .zip(scores[1..].iter().copied())
    {
        if score < best {
            best = score;
            selected = Some(candidate);
        }
    }
    (best.saturating_mul(4) <= baseline.saturating_mul(3)).then_some(selected?)
}

fn build_palette_plan(
    entries: Vec<[u8; 4]>,
    indices: Vec<u8>,
    width: usize,
) -> Result<PalettePlan, EncodeError> {
    let indices_per_pixel = match entries.len() {
        1..=2 => 8,
        3..=4 => 4,
        5..=16 => 2,
        _ => return Err(EncodeError::output_size_overflow()),
    };
    let indexed_width = width.div_ceil(indices_per_pixel);
    let height = indices.len() / width;
    let indexed_len = indexed_width
        .checked_mul(height)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(EncodeError::output_size_overflow)?;
    let mut indexed_rgba = Vec::new();
    indexed_rgba
        .try_reserve_exact(indexed_len)
        .map_err(|_| EncodeError::allocation_failed())?;
    let bits_per_index = 8 / indices_per_pixel;
    for row in indices.chunks_exact(width) {
        for packed_indices in row.chunks(indices_per_pixel) {
            let mut packed = 0_u8;
            for (position, index) in packed_indices.iter().copied().enumerate() {
                packed |= index << (position * bits_per_index);
            }
            indexed_rgba.extend_from_slice(&[0, packed, 0, 0]);
        }
    }
    Ok(PalettePlan {
        entries,
        indexed_rgba,
        indexed_width,
    })
}

const fn color_transform_delta(channel: u8, multiplier: i8) -> i16 {
    (channel as i8 as i16 * multiplier as i16) >> 5
}

const fn color_score(red: u8, blue: u8) -> u64 {
    signed_byte_magnitude(red) as u64 + signed_byte_magnitude(blue) as u64
}

const fn signed_byte_magnitude(value: u8) -> u8 {
    (value as i8).unsigned_abs()
}

#[cfg(test)]
#[path = "source_analysis_tests.rs"]
mod tests;
