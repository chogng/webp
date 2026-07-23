//! Spatial block geometry, token ownership, and per-group VP8L frequencies.

use super::EncodeError;
use super::spatial_cluster::cluster_tokens;
use super::token_stream::{EntropyFrequencies, TokenStream, token_span};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpatialProfile {
    Compact,
    LowLatency,
}

impl SpatialProfile {
    pub(crate) const fn block_pixels(self) -> usize {
        match self {
            Self::Compact => 128,
            Self::LowLatency => 256,
        }
    }

    pub(crate) const fn maximum_groups(self) -> usize {
        match self {
            Self::Compact => 64,
            Self::LowLatency => 16,
        }
    }

    pub(crate) const fn wire_block_bits(self) -> u8 {
        match self {
            Self::Compact => 5,
            Self::LowLatency => 6,
        }
    }
}

pub(crate) struct SpatialPlan {
    profile: SpatialProfile,
    image_width: usize,
    map_width: usize,
    group_map: Vec<u8>,
    frequencies: Vec<EntropyFrequencies>,
}

impl SpatialPlan {
    pub(crate) fn build(
        stream: &TokenStream,
        profile: SpatialProfile,
    ) -> Result<Self, EncodeError> {
        let geometry = stream.geometry();
        let clustered = cluster_tokens(stream, profile.block_pixels(), profile.maximum_groups())?;
        let mut frequencies = Vec::new();
        frequencies
            .try_reserve_exact(clustered.group_count)
            .map_err(|_| EncodeError::allocation_failed())?;
        for _ in 0..clustered.group_count {
            frequencies.push(EntropyFrequencies::for_color_cache(
                stream.color_cache_bits(),
            ));
        }
        let mut pixel = 0_usize;
        for &token in stream.tokens() {
            let block = block_index(
                pixel,
                geometry.width(),
                clustered.block_width,
                profile.block_pixels(),
            );
            frequencies[usize::from(clustered.assignments[block])].add_token(token)?;
            pixel = pixel
                .checked_add(token_span(token))
                .ok_or_else(EncodeError::output_size_overflow)?;
        }
        Ok(Self {
            profile,
            image_width: geometry.width(),
            map_width: clustered.block_width,
            group_map: clustered.assignments,
            frequencies,
        })
    }

    pub(crate) const fn map_width(&self) -> usize {
        self.map_width
    }

    pub(crate) fn group_map(&self) -> &[u8] {
        &self.group_map
    }

    pub(super) fn frequencies(&self) -> &[EntropyFrequencies] {
        &self.frequencies
    }

    pub(crate) fn group_for_pixel(&self, pixel: usize) -> usize {
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

#[cfg(test)]
#[path = "spatial_plan_tests.rs"]
mod tests;
