//! Spatial block geometry, token ownership, and per-group VP8L frequencies.

use super::spatial_cluster::{cluster_tokens, token_span};
use super::{
    CHANNEL_ALPHABET_SIZE, DISTANCE_ALPHABET_SIZE, EncodeError, EntropyFrequencies, EntropyToken,
    FIRST_CACHE_SYMBOL, GREEN_ALPHABET_SIZE, MAIN_GREEN_ALPHABET_SIZE, color_cache_size,
    increment_frequency, vp8l_prefix,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SpatialProfile {
    Compact,
    LowLatency,
}

impl SpatialProfile {
    pub(super) const fn block_pixels(self) -> usize {
        match self {
            Self::Compact => 128,
            Self::LowLatency => 256,
        }
    }

    pub(super) const fn maximum_groups(self) -> usize {
        match self {
            Self::Compact => 64,
            Self::LowLatency => 16,
        }
    }

    pub(super) const fn wire_block_bits(self) -> u8 {
        match self {
            Self::Compact => 5,
            Self::LowLatency => 6,
        }
    }
}

pub(super) struct SpatialPlan {
    profile: SpatialProfile,
    image_width: usize,
    map_width: usize,
    group_map: Vec<u8>,
    frequencies: Vec<EntropyFrequencies>,
}

impl SpatialPlan {
    pub(super) fn build(
        tokens: &[EntropyToken],
        width: usize,
        height: usize,
        color_cache_bits: u8,
        profile: SpatialProfile,
    ) -> Result<Self, EncodeError> {
        let clustered = cluster_tokens(
            tokens,
            width,
            height,
            profile.block_pixels(),
            profile.maximum_groups(),
        )?;
        let mut frequencies = Vec::new();
        frequencies
            .try_reserve_exact(clustered.group_count)
            .map_err(|_| EncodeError::allocation_failed())?;
        for _ in 0..clustered.group_count {
            frequencies.push(empty_frequencies(color_cache_bits));
        }
        let mut pixel = 0_usize;
        for &token in tokens {
            let block = block_index(pixel, width, clustered.block_width, profile.block_pixels());
            add_token(
                &mut frequencies[usize::from(clustered.assignments[block])],
                token,
            )?;
            pixel = pixel
                .checked_add(token_span(token))
                .ok_or_else(EncodeError::output_size_overflow)?;
        }
        Ok(Self {
            profile,
            image_width: width,
            map_width: clustered.block_width,
            group_map: clustered.assignments,
            frequencies,
        })
    }

    pub(super) const fn map_width(&self) -> usize {
        self.map_width
    }

    pub(super) fn group_map(&self) -> &[u8] {
        &self.group_map
    }

    pub(super) fn frequencies(&self) -> &[EntropyFrequencies] {
        &self.frequencies
    }

    pub(super) fn group_for_pixel(&self, pixel: usize) -> usize {
        let block = block_index(
            pixel,
            self.image_width,
            self.map_width,
            self.profile.block_pixels(),
        );
        usize::from(self.group_map[block])
    }
}

const fn block_index(pixel: usize, width: usize, block_width: usize, block_pixels: usize) -> usize {
    (pixel / width / block_pixels) * block_width + (pixel % width / block_pixels)
}

fn add_token(frequencies: &mut EntropyFrequencies, token: EntropyToken) -> Result<(), EncodeError> {
    match token {
        EntropyToken::Literal(rgba) => {
            increment_frequency(&mut frequencies.green, usize::from(rgba[1]))?;
            increment_frequency(&mut frequencies.red, usize::from(rgba[0]))?;
            increment_frequency(&mut frequencies.blue, usize::from(rgba[2]))?;
            increment_frequency(&mut frequencies.alpha, usize::from(rgba[3]))?;
        }
        EntropyToken::Cache(index) => {
            increment_frequency(&mut frequencies.green, FIRST_CACHE_SYMBOL + index)?;
        }
        EntropyToken::Copy { length } => {
            let (length_prefix, _) = vp8l_prefix(length, 24)?;
            let (distance_prefix, _) = vp8l_prefix(121, DISTANCE_ALPHABET_SIZE)?;
            increment_frequency(
                &mut frequencies.green,
                CHANNEL_ALPHABET_SIZE + length_prefix,
            )?;
            increment_frequency(&mut frequencies.distance, distance_prefix)?;
        }
    }
    Ok(())
}

fn empty_frequencies(color_cache_bits: u8) -> EntropyFrequencies {
    EntropyFrequencies {
        green: [0; MAIN_GREEN_ALPHABET_SIZE],
        green_len: GREEN_ALPHABET_SIZE + color_cache_size(color_cache_bits),
        red: [0; CHANNEL_ALPHABET_SIZE],
        blue: [0; CHANNEL_ALPHABET_SIZE],
        alpha: [0; CHANNEL_ALPHABET_SIZE],
        distance: [0; DISTANCE_ALPHABET_SIZE],
    }
}

#[cfg(test)]
#[path = "spatial_plan_tests.rs"]
mod tests;
