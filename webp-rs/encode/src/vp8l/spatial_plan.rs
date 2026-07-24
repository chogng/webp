//! Spatial block geometry, token ownership, and per-group VP8L frequencies.

use super::EncodeError;
use super::spatial_cluster::cluster_tokens;
use super::token_stream::{EntropyFrequencies, SpatialBlockStatistics, TokenStream, token_span};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpatialProfile {
    Compact,
    LowLatency,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SpatialGrid {
    block_pixels: usize,
    maximum_groups: usize,
    wire_block_bits: u8,
}

impl SpatialGrid {
    const fn new(block_pixels: usize, maximum_groups: usize, wire_block_bits: u8) -> Self {
        Self {
            block_pixels,
            maximum_groups,
            wire_block_bits,
        }
    }

    pub(crate) const fn block_pixels(self) -> usize {
        self.block_pixels
    }

    const fn maximum_groups(self) -> usize {
        self.maximum_groups
    }

    const fn wire_block_bits(self) -> u8 {
        self.wire_block_bits
    }
}

pub(crate) const fn fine_spatial_grid() -> SpatialGrid {
    SpatialGrid::new(32, 64, 3)
}

impl SpatialProfile {
    pub(crate) const fn grid(self) -> SpatialGrid {
        match self {
            Self::Compact => SpatialGrid::new(128, 64, 5),
            Self::LowLatency => SpatialGrid::new(256, 16, 6),
        }
    }

    pub(crate) const fn wire_block_bits(self) -> u8 {
        self.grid().wire_block_bits()
    }
}

pub(crate) struct SpatialPlan {
    grid: SpatialGrid,
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
        Self::build_for_grid(stream, profile.grid())
    }

    pub(crate) fn build_for_grid(
        stream: &TokenStream,
        grid: SpatialGrid,
    ) -> Result<Self, EncodeError> {
        let statistics = stream.spatial_statistics(grid.block_pixels())?;
        Self::build_for_grid_with_statistics(stream, grid, &statistics)
    }

    pub(super) fn build_for_grid_with_statistics(
        stream: &TokenStream,
        grid: SpatialGrid,
        statistics: &SpatialBlockStatistics,
    ) -> Result<Self, EncodeError> {
        let geometry = stream.geometry();
        let clustered = cluster_tokens(statistics, grid.maximum_groups())?;
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
                grid.block_pixels(),
            );
            frequencies[usize::from(clustered.assignments[block])].add_token(token)?;
            pixel = pixel
                .checked_add(token_span(token))
                .ok_or_else(EncodeError::output_size_overflow)?;
        }
        Ok(Self {
            grid,
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
            self.grid.block_pixels(),
        );
        usize::from(self.group_map[block])
    }

    pub(crate) fn group_count(&self) -> usize {
        self.frequencies.len()
    }

    pub(crate) fn decode_group_runs(&self) -> Result<usize, EncodeError> {
        let block_pixels = self.grid.block_pixels();
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

    pub(crate) const fn wire_block_bits(&self) -> u8 {
        self.grid.wire_block_bits()
    }
}

const fn block_index(pixel: usize, width: usize, block_width: usize, block_pixels: usize) -> usize {
    (pixel / width / block_pixels) * block_width + (pixel % width / block_pixels)
}

#[cfg(test)]
#[path = "spatial_plan_tests.rs"]
mod tests;
