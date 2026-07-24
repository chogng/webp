//! Deterministic bounded clustering for VP8L coarse spatial groups.

use std::cmp::Reverse;
use std::collections::BTreeMap;

use super::EncodeError;
use super::token_stream::{BlockHistogram, COARSE_HISTOGRAM_BINS, SpatialBlockStatistics};

impl BlockHistogram {
    fn signature(self) -> Signature {
        let total = self.literals().saturating_add(self.branches()).max(1);
        let literals = self.literals().max(1);
        let mut bins = [0_u8; 4 * COARSE_HISTOGRAM_BINS + 1];
        for (channel, histogram) in self.channels().iter().enumerate() {
            for (bin, &count) in histogram.iter().enumerate() {
                bins[channel * COARSE_HISTOGRAM_BINS + bin] =
                    ((count * 15) / literals).min(15) as u8;
            }
        }
        bins[4 * COARSE_HISTOGRAM_BINS] =
            (self.branches().saturating_mul(15) / total).min(15) as u8;
        Signature { bins }
    }
}

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
struct Signature {
    bins: [u8; 4 * COARSE_HISTOGRAM_BINS + 1],
}

impl Signature {
    fn distance(self, other: Self) -> u16 {
        self.bins
            .iter()
            .zip(other.bins)
            .map(|(&left, right)| u16::from(left.abs_diff(right)))
            .sum()
    }
}

pub(crate) struct ClusteredMap {
    pub(crate) block_width: usize,
    pub(crate) assignments: Vec<u8>,
    pub(crate) group_count: usize,
}

pub(crate) fn cluster_tokens(
    statistics: &SpatialBlockStatistics,
    maximum_groups: usize,
) -> Result<ClusteredMap, EncodeError> {
    let mut weights = BTreeMap::<Signature, u64>::new();
    for block in statistics.blocks() {
        if !block.is_empty() {
            let weight = weights.entry(block.signature()).or_default();
            *weight = weight
                .checked_add(block.token_count())
                .ok_or_else(EncodeError::output_size_overflow)?;
        }
    }
    let mut ranked = weights.into_iter().collect::<Vec<_>>();
    ranked.sort_unstable_by_key(|(signature, weight)| (Reverse(*weight), *signature));
    let seeds = ranked
        .into_iter()
        .take(maximum_groups)
        .map(|(signature, _)| signature)
        .collect::<Vec<_>>();
    if seeds.is_empty() {
        return Err(EncodeError::output_size_overflow());
    }

    let mut assignments = Vec::new();
    assignments
        .try_reserve_exact(statistics.blocks().len())
        .map_err(|_| EncodeError::allocation_failed())?;
    assignments.resize(statistics.blocks().len(), u8::MAX);
    for (index, block) in statistics.blocks().iter().enumerate() {
        if block.is_empty() {
            continue;
        }
        let signature = block.signature();
        assignments[index] = seeds
            .iter()
            .enumerate()
            .min_by_key(|&(seed, candidate)| (signature.distance(*candidate), seed))
            .and_then(|(seed, _)| u8::try_from(seed).ok())
            .ok_or_else(EncodeError::output_size_overflow)?;
    }
    fill_empty_blocks(&mut assignments, statistics.block_width());
    let group_count = compact_groups(&mut assignments, seeds.len())?;
    Ok(ClusteredMap {
        block_width: statistics.block_width(),
        assignments,
        group_count,
    })
}

fn fill_empty_blocks(assignments: &mut [u8], block_width: usize) {
    for index in 0..assignments.len() {
        if assignments[index] != u8::MAX {
            continue;
        }
        assignments[index] = if !index.is_multiple_of(block_width) {
            assignments[index - 1]
        } else if index >= block_width {
            assignments[index - block_width]
        } else {
            0
        };
    }
}

fn compact_groups(assignments: &mut [u8], seed_count: usize) -> Result<usize, EncodeError> {
    let mut used = vec![false; seed_count];
    for &group in assignments.iter() {
        used[usize::from(group)] = true;
    }
    let mut remap = vec![u8::MAX; seed_count];
    let mut compact = 0_usize;
    for (old, is_used) in used.into_iter().enumerate() {
        if is_used {
            remap[old] = u8::try_from(compact).map_err(|_| EncodeError::output_size_overflow())?;
            compact += 1;
        }
    }
    for group in assignments {
        *group = remap[usize::from(*group)];
    }
    Ok(compact)
}

#[cfg(test)]
#[path = "spatial_cluster_tests.rs"]
mod tests;
