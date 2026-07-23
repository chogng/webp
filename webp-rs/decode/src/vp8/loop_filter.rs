//! Scalar VP8 loop-filter strength derivation and edge filters.

use crate::vp8::FilterHeader;
use crate::vp8::SegmentHeader;
pub use webp_dsp::LoopFilterStrength;
pub use webp_dsp::filter_normal_edge;
pub use webp_dsp::filter_simple_edge;

#[cfg(test)]
#[path = "loop_filter_tests.rs"]
mod tests;

/// Derives VP8 loop-filter strengths for four segments and both luma mode
/// classes (`[segment][0 = 16×16, 1 = 4×4]`).
///
/// Segmentation and filter deltas use the same inheritance and clamping rules
/// as libwebp's frame initialization. Reference delta zero is the only one
/// applicable to WebP still-image key frames.
#[must_use]
pub fn derive_loop_filter_strengths(
    filter: &FilterHeader,
    segments: &SegmentHeader,
) -> [[LoopFilterStrength; 2]; 4] {
    // VP8 enables the whole in-loop filter from the base header level. As in
    // libwebp, a zero level disables filtering before segment or mode deltas
    // are considered.
    if filter.level == 0 {
        return [[LoopFilterStrength::default(); 2]; 4];
    }
    std::array::from_fn(|segment| {
        std::array::from_fn(|mode_class| {
            let mut level = if segments.enabled {
                if segments.absolute_delta {
                    segments.filter_strength[segment]
                } else {
                    i32::from(filter.level) + segments.filter_strength[segment]
                }
            } else {
                i32::from(filter.level)
            };
            if filter.use_deltas {
                level += filter.ref_deltas[0];
                if mode_class == 1 {
                    level += filter.mode_deltas[0];
                }
            }
            let level = level.clamp(0, 63) as u8;
            if level == 0 {
                return LoopFilterStrength::default();
            }
            let mut inner_limit = level;
            if filter.sharpness > 0 {
                inner_limit >>= if filter.sharpness > 4 { 2 } else { 1 };
                inner_limit = inner_limit.min(9 - filter.sharpness);
            }
            inner_limit = inner_limit.max(1);
            LoopFilterStrength {
                level,
                inner_limit,
                edge_limit: level.saturating_mul(2).saturating_add(inner_limit),
                hev_threshold: if level >= 40 {
                    2
                } else if level >= 15 {
                    1
                } else {
                    0
                },
            }
        })
    })
}

/// All plane and control state required to filter one reconstructed macroblock.
pub(crate) struct MacroblockFilter<'a> {
    pub y: &'a mut [u8],
    pub u: &'a mut [u8],
    pub v: &'a mut [u8],
    pub y_stride: usize,
    pub uv_stride: usize,
    pub macroblock_x: usize,
    pub macroblock_y: usize,
    pub simple: bool,
    pub strength: LoopFilterStrength,
    pub filters_inner: bool,
}

/// Filters the outer and internal edges of one reconstructed macroblock.
///
/// The caller invokes this after the complete macroblock row has been
/// reconstructed, matching VP8's row-filtering order. The planes are padded
/// to macroblock boundaries, so every supplied macroblock has a full 16×16
/// luma and 8×8 chroma region.
pub(crate) fn filter_macroblock(filter: MacroblockFilter<'_>) {
    if filter.strength.edge_limit == 0 {
        return;
    }
    let y_origin = filter.macroblock_y * 16 * filter.y_stride + filter.macroblock_x * 16;
    let uv_origin = filter.macroblock_y * 8 * filter.uv_stride + filter.macroblock_x * 8;
    let has_left = filter.macroblock_x > 0;
    let has_top = filter.macroblock_y > 0;
    if filter.simple {
        filter_simple_luma_macroblock(
            filter.y,
            filter.y_stride,
            y_origin,
            has_left,
            has_top,
            filter.strength,
            filter.filters_inner,
        );
    } else {
        let y_filter = PlaneMacroblockFilter::new(
            filter.y_stride,
            y_origin,
            16,
            has_left,
            has_top,
            filter.strength,
            filter.filters_inner,
        );
        let uv_filter = PlaneMacroblockFilter::new(
            filter.uv_stride,
            uv_origin,
            8,
            has_left,
            has_top,
            filter.strength,
            filter.filters_inner,
        );
        filter_normal_plane_macroblock(filter.y, y_filter);
        filter_normal_plane_macroblock(filter.u, uv_filter);
        filter_normal_plane_macroblock(filter.v, uv_filter);
    }
}

fn filter_simple_luma_macroblock(
    plane: &mut [u8],
    stride: usize,
    origin: usize,
    has_left: bool,
    has_top: bool,
    strength: LoopFilterStrength,
    filters_inner: bool,
) {
    if has_left {
        for row in 0..16 {
            let _ = filter_simple_edge(
                plane,
                origin + row * stride,
                1,
                strength.edge_limit.saturating_add(4),
            );
        }
    }
    if filters_inner {
        for edge in [4, 8, 12] {
            for row in 0..16 {
                let _ =
                    filter_simple_edge(plane, origin + row * stride + edge, 1, strength.edge_limit);
            }
        }
    }
    if has_top {
        for column in 0..16 {
            let _ = filter_simple_edge(
                plane,
                origin + column,
                stride,
                strength.edge_limit.saturating_add(4),
            );
        }
    }
    if filters_inner {
        for edge in [4, 8, 12] {
            for column in 0..16 {
                let _ = filter_simple_edge(
                    plane,
                    origin + edge * stride + column,
                    stride,
                    strength.edge_limit,
                );
            }
        }
    }
}

#[derive(Clone, Copy)]
struct PlaneMacroblockFilter {
    stride: usize,
    origin: usize,
    size: usize,
    has_left: bool,
    has_top: bool,
    strength: LoopFilterStrength,
    filters_inner: bool,
}

impl PlaneMacroblockFilter {
    const fn new(
        stride: usize,
        origin: usize,
        size: usize,
        has_left: bool,
        has_top: bool,
        strength: LoopFilterStrength,
        filters_inner: bool,
    ) -> Self {
        Self {
            stride,
            origin,
            size,
            has_left,
            has_top,
            strength,
            filters_inner,
        }
    }
}

fn filter_normal_plane_macroblock(plane: &mut [u8], filter: PlaneMacroblockFilter) {
    let outer_strength = LoopFilterStrength {
        edge_limit: filter.strength.edge_limit.saturating_add(4),
        ..filter.strength
    };
    if filter.has_left {
        for row in 0..filter.size {
            let _ = filter_normal_edge(
                plane,
                filter.origin + row * filter.stride,
                1,
                outer_strength,
                true,
            );
        }
    }
    if filter.filters_inner {
        for edge in (4..filter.size).step_by(4) {
            for row in 0..filter.size {
                let _ = filter_normal_edge(
                    plane,
                    filter.origin + row * filter.stride + edge,
                    1,
                    filter.strength,
                    false,
                );
            }
        }
    }
    if filter.has_top {
        for column in 0..filter.size {
            let _ = filter_normal_edge(
                plane,
                filter.origin + column,
                filter.stride,
                outer_strength,
                true,
            );
        }
    }
    if filter.filters_inner {
        for edge in (4..filter.size).step_by(4) {
            for column in 0..filter.size {
                let _ = filter_normal_edge(
                    plane,
                    filter.origin + edge * filter.stride + column,
                    filter.stride,
                    filter.strength,
                    false,
                );
            }
        }
    }
}
