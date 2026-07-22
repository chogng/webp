//! Deterministic bounded clustering for VP8L coarse spatial groups.

use std::cmp::Reverse;
use std::collections::BTreeMap;

use super::{EncodeError, EntropyToken};

#[derive(Clone, Copy, Default)]
struct Majority {
    symbol: u8,
    balance: i32,
}

impl Majority {
    fn add(&mut self, symbol: u8) -> Result<(), EncodeError> {
        if self.balance == 0 {
            self.symbol = symbol;
            self.balance = 1;
        } else if self.symbol == symbol {
            self.balance = self
                .balance
                .checked_add(1)
                .ok_or_else(EncodeError::output_size_overflow)?;
        } else {
            self.balance -= 1;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Default)]
struct BlockHistogram {
    channels: [Majority; 4],
    literals: u32,
    branches: u32,
}

impl BlockHistogram {
    fn add(&mut self, token: EntropyToken) -> Result<(), EncodeError> {
        match token {
            EntropyToken::Literal(rgba) => {
                for (majority, symbol) in self.channels.iter_mut().zip(rgba) {
                    majority.add(symbol)?;
                }
                self.literals = self
                    .literals
                    .checked_add(1)
                    .ok_or_else(EncodeError::output_size_overflow)?;
            }
            EntropyToken::Cache(_) | EntropyToken::Copy { .. } => {
                self.branches = self
                    .branches
                    .checked_add(1)
                    .ok_or_else(EncodeError::output_size_overflow)?;
            }
        }
        Ok(())
    }

    const fn is_empty(self) -> bool {
        self.literals == 0 && self.branches == 0
    }

    const fn token_count(self) -> u64 {
        self.literals as u64 + self.branches as u64
    }

    fn signature(self) -> Signature {
        let total = self.literals.saturating_add(self.branches).max(1);
        Signature {
            bins: [
                self.channels[0].symbol >> 5,
                self.channels[1].symbol >> 5,
                self.channels[2].symbol >> 5,
                self.channels[3].symbol >> 5,
                (self.branches.saturating_mul(4) / total).min(3) as u8,
            ],
        }
    }
}

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
struct Signature {
    bins: [u8; 5],
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

pub(super) struct ClusteredMap {
    pub(super) block_width: usize,
    pub(super) assignments: Vec<u8>,
    pub(super) group_count: usize,
}

pub(super) fn cluster_tokens(
    tokens: &[EntropyToken],
    width: usize,
    height: usize,
    block_pixels: usize,
    maximum_groups: usize,
) -> Result<ClusteredMap, EncodeError> {
    let block_width = width.div_ceil(block_pixels);
    let block_height = height.div_ceil(block_pixels);
    let block_count = block_width
        .checked_mul(block_height)
        .ok_or_else(EncodeError::output_size_overflow)?;
    let mut blocks = Vec::new();
    blocks
        .try_reserve_exact(block_count)
        .map_err(|_| EncodeError::allocation_failed())?;
    blocks.resize(block_count, BlockHistogram::default());

    let mut pixel = 0_usize;
    for &token in tokens {
        let block = block_index(pixel, width, block_width, block_pixels);
        blocks[block].add(token)?;
        pixel = pixel
            .checked_add(token_span(token))
            .ok_or_else(EncodeError::output_size_overflow)?;
    }

    let mut weights = BTreeMap::<Signature, u64>::new();
    for block in &blocks {
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
        .try_reserve_exact(block_count)
        .map_err(|_| EncodeError::allocation_failed())?;
    assignments.resize(block_count, u8::MAX);
    for (index, block) in blocks.iter().enumerate() {
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
    fill_empty_blocks(&mut assignments, block_width);
    let group_count = compact_groups(&mut assignments, seeds.len())?;
    Ok(ClusteredMap {
        block_width,
        assignments,
        group_count,
    })
}

pub(super) const fn token_span(token: EntropyToken) -> usize {
    match token {
        EntropyToken::Literal(_) | EntropyToken::Cache(_) => 1,
        EntropyToken::Copy { length } => length,
    }
}

const fn block_index(pixel: usize, width: usize, block_width: usize, block_pixels: usize) -> usize {
    (pixel / width / block_pixels) * block_width + (pixel % width / block_pixels)
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
