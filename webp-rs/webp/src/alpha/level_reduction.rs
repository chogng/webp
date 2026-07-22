//! Encoder-side alpha level reduction.

use crate::alpha::AlphaEncodeError;

const SYMBOLS: usize = 256;
const MAX_ITERATIONS: usize = 6;

pub(super) fn quantize(samples: &[u8], quality: u8) -> Result<Vec<u8>, AlphaEncodeError> {
    debug_assert!(quality < 100);
    let levels = if quality <= 70 {
        2 + usize::from(quality / 5)
    } else {
        16 + usize::from(quality - 70) * 8
    };
    quantize_to_levels(samples, levels)
}

fn quantize_to_levels(samples: &[u8], levels: usize) -> Result<Vec<u8>, AlphaEncodeError> {
    let mut frequencies = [0_u64; SYMBOLS];
    let mut minimum = u8::MAX;
    let mut maximum = u8::MIN;
    for &sample in samples {
        frequencies[usize::from(sample)] += 1;
        minimum = minimum.min(sample);
        maximum = maximum.max(sample);
    }
    let distinct = frequencies.iter().filter(|&&count| count != 0).count();
    if distinct <= levels {
        return copy(samples);
    }

    let minimum = usize::from(minimum);
    let maximum = usize::from(maximum);
    let mut assignments = [0_usize; SYMBOLS];
    let mut centroids = [0_f64; SYMBOLS];
    for (index, centroid) in centroids.iter_mut().take(levels).enumerate() {
        *centroid =
            minimum as f64 + (maximum - minimum) as f64 * index as f64 / (levels - 1) as f64;
    }

    let threshold = 1e-4 * samples.len() as f64;
    let mut last_error = 1e38_f64;
    for _ in 0..MAX_ITERATIONS {
        let mut sums = [0_f64; SYMBOLS];
        let mut counts = [0_f64; SYMBOLS];
        let mut slot = 0;
        for symbol in minimum..=maximum {
            while slot + 1 < levels && 2.0 * symbol as f64 > centroids[slot] + centroids[slot + 1] {
                slot += 1;
            }
            if frequencies[symbol] != 0 {
                sums[slot] += symbol as f64 * frequencies[symbol] as f64;
                counts[slot] += frequencies[symbol] as f64;
            }
            assignments[symbol] = slot;
        }
        for slot in 1..levels - 1 {
            if counts[slot] != 0.0 {
                centroids[slot] = sums[slot] / counts[slot];
            }
        }
        let error = (minimum..=maximum)
            .map(|symbol| {
                let delta = symbol as f64 - centroids[assignments[symbol]];
                frequencies[symbol] as f64 * delta * delta
            })
            .sum::<f64>();
        if last_error - error < threshold {
            break;
        }
        last_error = error;
    }

    let mut map = [0_u8; SYMBOLS];
    for symbol in minimum..=maximum {
        map[symbol] = (centroids[assignments[symbol]] + 0.5) as u8;
    }
    let mut output = Vec::new();
    output
        .try_reserve_exact(samples.len())
        .map_err(|_| AlphaEncodeError::AllocationFailed)?;
    output.extend(samples.iter().map(|&sample| map[usize::from(sample)]));
    Ok(output)
}

fn copy(samples: &[u8]) -> Result<Vec<u8>, AlphaEncodeError> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(samples.len())
        .map_err(|_| AlphaEncodeError::AllocationFailed)?;
    output.extend_from_slice(samples);
    Ok(output)
}

#[cfg(test)]
#[path = "level_reduction_tests.rs"]
mod tests;
