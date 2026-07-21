//! Forward ALPH filters and encoder-side filter selection.

use crate::AlphaEncodeError;
use crate::AlphaFilter;

/// Policy used to select the spatial filter written into an `ALPH` header.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AlphaFilterSelection {
    /// Encode exactly one specified wire filter.
    Fixed(AlphaFilter),
    /// Estimate a promising filter and compare it with no filtering.
    Fast,
    /// Encode all four filters and retain the smallest payload.
    Best,
}

impl Default for AlphaFilterSelection {
    fn default() -> Self {
        Self::Fixed(AlphaFilter::None)
    }
}

impl From<AlphaFilter> for AlphaFilterSelection {
    fn from(filter: AlphaFilter) -> Self {
        Self::Fixed(filter)
    }
}

pub(super) fn candidates(
    samples: &[u8],
    width: usize,
    height: usize,
    selection: AlphaFilterSelection,
) -> Vec<AlphaFilter> {
    match selection {
        AlphaFilterSelection::Fixed(filter) => vec![filter],
        AlphaFilterSelection::Best => all_filters().to_vec(),
        AlphaFilterSelection::Fast => {
            let estimated = estimate(samples, width, height);
            if estimated == AlphaFilter::None {
                vec![AlphaFilter::None]
            } else {
                // libwebp's ordinary effort level compares the quick estimate
                // with no filtering, which protects high-entropy planes.
                vec![AlphaFilter::None, estimated]
            }
        }
    }
}

pub(super) fn apply(
    samples: &[u8],
    width: usize,
    filter: AlphaFilter,
) -> Result<Vec<u8>, AlphaEncodeError> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(samples.len())
        .map_err(|_| AlphaEncodeError::AllocationFailed)?;
    for (index, &sample) in samples.iter().enumerate() {
        let x = index % width;
        let y = index / width;
        let left = if x != 0 { samples[index - 1] } else { 0 };
        let top = if y != 0 { samples[index - width] } else { 0 };
        let top_left = if x != 0 && y != 0 {
            samples[index - width - 1]
        } else {
            0
        };
        let predictor = match filter {
            AlphaFilter::None => 0,
            AlphaFilter::Horizontal => {
                if x == 0 {
                    top
                } else {
                    left
                }
            }
            AlphaFilter::Vertical => {
                if y == 0 {
                    left
                } else {
                    top
                }
            }
            AlphaFilter::Gradient => {
                if x == 0 {
                    top
                } else if y == 0 {
                    left
                } else {
                    gradient(left, top, top_left)
                }
            }
        };
        output.push(sample.wrapping_sub(predictor));
    }
    Ok(output)
}

fn estimate(samples: &[u8], width: usize, height: usize) -> AlphaFilter {
    const BIN_COUNT: usize = 16;
    let mut bins = [[false; BIN_COUNT]; 4];
    let mut y = 2;
    while y + 1 < height {
        let row = y * width;
        let mut mean = usize::from(samples[row]);
        let mut x = 2;
        while x + 1 < width {
            let index = row + x;
            let sample = usize::from(samples[index]);
            let left = usize::from(samples[index - 1]);
            let top = usize::from(samples[index - width]);
            let top_left = usize::from(samples[index - width - 1]);
            let gradient = (left + top).saturating_sub(top_left).min(255);
            for (filter, difference) in [
                sample.abs_diff(mean),
                sample.abs_diff(left),
                sample.abs_diff(top),
                sample.abs_diff(gradient),
            ]
            .into_iter()
            .enumerate()
            {
                bins[filter][difference >> 4] = true;
            }
            mean = (3 * mean + sample + 2) >> 2;
            x += 2;
        }
        y += 2;
    }

    let best = bins
        .iter()
        .enumerate()
        .min_by_key(|(_, bins)| {
            bins.iter()
                .enumerate()
                .filter_map(|(index, used)| used.then_some(index))
                .sum::<usize>()
        })
        .map(|(index, _)| index)
        .unwrap_or(0);
    all_filters()[best]
}

const fn all_filters() -> [AlphaFilter; 4] {
    [
        AlphaFilter::None,
        AlphaFilter::Horizontal,
        AlphaFilter::Vertical,
        AlphaFilter::Gradient,
    ]
}

#[inline]
fn gradient(left: u8, top: u8, top_left: u8) -> u8 {
    (left as i16 + top as i16 - top_left as i16).clamp(0, 255) as u8
}

#[cfg(test)]
#[path = "encode_filter_tests.rs"]
mod tests;
