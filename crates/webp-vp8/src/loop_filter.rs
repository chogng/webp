//! Scalar VP8 loop-filter strength derivation and edge filters.

use crate::{FilterHeader, SegmentHeader};

/// Precomputed VP8 loop-filter controls for one segment and luma mode class.
///
/// The values match the scalar controls used by VP8's simple and normal
/// in-loop filters. `edge_limit == 0` disables filtering for this class.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct LoopFilterStrength {
    pub level: u8,
    pub inner_limit: u8,
    pub edge_limit: u8,
    pub hev_threshold: u8,
}

impl LoopFilterStrength {
    /// Whether this macroblock needs filtering at its internal 4×4 edges.
    #[must_use]
    pub const fn filters_inner(self, is_i4x4: bool, skip: bool) -> bool {
        self.edge_limit != 0 && (is_i4x4 || !skip)
    }
}

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

/// Applies VP8's simple two-tap filter across one plane edge.
///
/// `q0` is the first sample after the edge and `step` is the distance between
/// adjacent samples across it. The function returns `false` for a rejected
/// edge or an out-of-bounds location without modifying the plane.
#[must_use]
pub fn filter_simple_edge(plane: &mut [u8], q0: usize, step: usize, threshold: u8) -> bool {
    if step == 0 {
        return false;
    }
    let Some(p1) = q0.checked_sub(step.saturating_mul(2)) else {
        return false;
    };
    let Some(p0) = q0.checked_sub(step) else {
        return false;
    };
    let Some(q1) = q0.checked_add(step) else {
        return false;
    };
    if q1 >= plane.len()
        || !needs_filter(
            plane[p1],
            plane[p0],
            plane[q0],
            plane[q1],
            2 * i32::from(threshold) + 1,
        )
    {
        return false;
    }
    filter_two(plane, p1, p0, q0, q1);
    true
}

/// Applies VP8's normal four- or six-tap filter across one plane edge.
///
/// `macroblock_edge` selects the six-tap outer-edge filter; otherwise the
/// four-tap internal-edge filter is used. As with [`filter_simple_edge`], an
/// invalid or rejected edge returns `false` without mutating the plane.
#[must_use]
pub fn filter_normal_edge(
    plane: &mut [u8],
    q0: usize,
    step: usize,
    strength: LoopFilterStrength,
    macroblock_edge: bool,
) -> bool {
    if step == 0 {
        return false;
    }
    let Some(p3) = q0.checked_sub(step.saturating_mul(4)) else {
        return false;
    };
    let Some(p2) = q0.checked_sub(step.saturating_mul(3)) else {
        return false;
    };
    let Some(p1) = q0.checked_sub(step.saturating_mul(2)) else {
        return false;
    };
    let Some(p0) = q0.checked_sub(step) else {
        return false;
    };
    let Some(q1) = q0.checked_add(step) else {
        return false;
    };
    let Some(q2) = q0.checked_add(step.saturating_mul(2)) else {
        return false;
    };
    let Some(q3) = q0.checked_add(step.saturating_mul(3)) else {
        return false;
    };
    if q3 >= plane.len()
        || strength.edge_limit == 0
        || !needs_filter_normal(
            [plane[p3], plane[p2], plane[p1], plane[p0]],
            [plane[q0], plane[q1], plane[q2], plane[q3]],
            strength,
        )
    {
        return false;
    }
    if high_edge_variance(
        plane[p1],
        plane[p0],
        plane[q0],
        plane[q1],
        strength.hev_threshold,
    ) {
        filter_two(plane, p1, p0, q0, q1);
    } else if macroblock_edge {
        filter_six(plane, p2, p1, p0, q0, q1, q2);
    } else {
        filter_four(plane, p1, p0, q0, q1);
    }
    true
}

fn needs_filter(p1: u8, p0: u8, q0: u8, q1: u8, threshold: i32) -> bool {
    4 * abs_diff(p0, q0) + abs_diff(p1, q1) <= threshold
}

fn needs_filter_normal(p: [u8; 4], q: [u8; 4], strength: LoopFilterStrength) -> bool {
    needs_filter(
        p[2],
        p[3],
        q[0],
        q[1],
        2 * i32::from(strength.edge_limit) + 1,
    ) && abs_diff(p[0], p[1]) <= i32::from(strength.inner_limit)
        && abs_diff(p[1], p[2]) <= i32::from(strength.inner_limit)
        && abs_diff(p[2], p[3]) <= i32::from(strength.inner_limit)
        && abs_diff(q[3], q[2]) <= i32::from(strength.inner_limit)
        && abs_diff(q[2], q[1]) <= i32::from(strength.inner_limit)
        && abs_diff(q[1], q[0]) <= i32::from(strength.inner_limit)
}

fn high_edge_variance(p1: u8, p0: u8, q0: u8, q1: u8, threshold: u8) -> bool {
    abs_diff(p1, p0) > i32::from(threshold) || abs_diff(q1, q0) > i32::from(threshold)
}

fn filter_two(plane: &mut [u8], p1: usize, p0: usize, q0: usize, q1: usize) {
    let delta = 3 * (i32::from(plane[q0]) - i32::from(plane[p0]))
        + clip_signed(i32::from(plane[p1]) - i32::from(plane[q1]));
    let a1 = clip_signed((delta + 4) >> 3);
    let a2 = clip_signed((delta + 3) >> 3);
    plane[p0] = clip_sample(i32::from(plane[p0]) + a2);
    plane[q0] = clip_sample(i32::from(plane[q0]) - a1);
}

fn filter_four(plane: &mut [u8], p1: usize, p0: usize, q0: usize, q1: usize) {
    let delta = 3 * (i32::from(plane[q0]) - i32::from(plane[p0]));
    let a1 = clip_signed((delta + 4) >> 3);
    let a2 = clip_signed((delta + 3) >> 3);
    let a3 = (a1 + 1) >> 1;
    plane[p1] = clip_sample(i32::from(plane[p1]) + a3);
    plane[p0] = clip_sample(i32::from(plane[p0]) + a2);
    plane[q0] = clip_sample(i32::from(plane[q0]) - a1);
    plane[q1] = clip_sample(i32::from(plane[q1]) - a3);
}

fn filter_six(plane: &mut [u8], p2: usize, p1: usize, p0: usize, q0: usize, q1: usize, q2: usize) {
    let delta = clip_signed(
        3 * (i32::from(plane[q0]) - i32::from(plane[p0]))
            + clip_signed(i32::from(plane[p1]) - i32::from(plane[q1])),
    );
    let a1 = (27 * delta + 63) >> 7;
    let a2 = (18 * delta + 63) >> 7;
    let a3 = (9 * delta + 63) >> 7;
    plane[p2] = clip_sample(i32::from(plane[p2]) + a3);
    plane[p1] = clip_sample(i32::from(plane[p1]) + a2);
    plane[p0] = clip_sample(i32::from(plane[p0]) + a1);
    plane[q0] = clip_sample(i32::from(plane[q0]) - a1);
    plane[q1] = clip_sample(i32::from(plane[q1]) - a2);
    plane[q2] = clip_sample(i32::from(plane[q2]) - a3);
}

fn abs_diff(left: u8, right: u8) -> i32 {
    (i32::from(left) - i32::from(right)).abs()
}

fn clip_signed(value: i32) -> i32 {
    value.clamp(-128, 127)
}

fn clip_sample(value: i32) -> u8 {
    value.clamp(0, 255) as u8
}
