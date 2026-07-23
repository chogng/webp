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
    image_height: usize,
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
        let clustered = cluster_tokens(
            stream.spatial_blocks(profile.block_pixels())?,
            profile.maximum_groups(),
        )?;
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
            image_height: geometry.height(),
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

    pub(crate) fn group_count(&self) -> usize {
        self.frequencies.len()
    }

    pub(crate) fn decode_group_runs(&self) -> Result<usize, EncodeError> {
        let block_pixels = self.profile.block_pixels();
        let mut runs = 0_usize;
        for (block_y, map_row) in self.group_map.chunks_exact(self.map_width).enumerate() {
            let row_runs = 1_usize + map_row.windows(2).filter(|pair| pair[0] != pair[1]).count();
            let source_y = block_y
                .checked_mul(block_pixels)
                .ok_or_else(EncodeError::output_size_overflow)?;
            let source_rows = block_pixels.min(
                self.image_height
                    .checked_sub(source_y)
                    .ok_or_else(EncodeError::output_size_overflow)?,
            );
            runs = runs
                .checked_add(
                    row_runs
                        .checked_mul(source_rows)
                        .ok_or_else(EncodeError::output_size_overflow)?,
                )
                .ok_or_else(EncodeError::output_size_overflow)?;
        }
        Ok(runs)
    }

    pub(crate) fn table_map_bytes(&self) -> Result<usize, EncodeError> {
        self.group_count()
            .checked_mul(5 * 2048)
            .and_then(|bytes| bytes.checked_add(self.group_map.len() * 2))
            .ok_or_else(EncodeError::output_size_overflow)
    }
}

const fn block_index(pixel: usize, width: usize, block_width: usize, block_pixels: usize) -> usize {
    (pixel / width / block_pixels) * block_width + (pixel % width / block_pixels)
}

#[cfg(test)]
#[path = "spatial_plan_tests.rs"]
mod tests;
