//! VP8 scalar intra-prediction kernels and neighbour state.

#[cfg(test)]
#[path = "vp8_prediction_tests.rs"]
mod tests;

/// One VP8 intra 4×4 prediction mode.
///
/// Numeric values match VP8's B-mode entropy contexts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Intra4Mode {
    Dc = 0,
    TrueMotion = 1,
    Vertical = 2,
    Horizontal = 3,
    DiagonalDownRight = 4,
    VerticalRight = 5,
    DiagonalDownLeft = 6,
    VerticalLeft = 7,
    HorizontalDown = 8,
    HorizontalUp = 9,
}

/// One of VP8's four 16×16 luma prediction modes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Intra16Mode {
    Dc,
    Vertical,
    Horizontal,
    TrueMotion,
}

/// One of VP8's four chroma prediction modes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChromaMode {
    Dc,
    Vertical,
    Horizontal,
    TrueMotion,
}

/// Reconstructed YUV samples for one VP8 16×16 macroblock.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MacroblockPixels {
    pub y: [u8; 256],
    pub u: [u8; 64],
    pub v: [u8; 64],
}

/// Already-reconstructed samples adjacent to one macroblock.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct MacroblockPredictionEdges {
    pub top_y: Option<[u8; 16]>,
    /// Four samples immediately right of `top_y`, needed by B_PRED.
    pub top_right_y: Option<[u8; 4]>,
    pub left_y: Option<[u8; 16]>,
    pub top_left_y: u8,
    pub top_u: Option<[u8; 8]>,
    pub left_u: Option<[u8; 8]>,
    pub top_left_u: u8,
    pub top_v: Option<[u8; 8]>,
    pub left_v: Option<[u8; 8]>,
    pub top_left_v: u8,
}

/// Builds a 16×16-luma/8×8-chroma VP8 intra prediction.
#[must_use]
pub fn predict_intra16_macroblock(
    luma_mode: Intra16Mode,
    chroma_mode: ChromaMode,
    edges: MacroblockPredictionEdges,
) -> MacroblockPixels {
    let mut prediction = MacroblockPixels {
        y: [0; 256],
        u: [0; 64],
        v: [0; 64],
    };
    predict_plane(
        &mut prediction.y,
        luma_mode.into(),
        edges.top_y,
        edges.left_y,
        edges.top_left_y,
    );
    predict_plane(
        &mut prediction.u,
        chroma_mode.into(),
        edges.top_u,
        edges.left_u,
        edges.top_left_u,
    );
    predict_plane(
        &mut prediction.v,
        chroma_mode.into(),
        edges.top_v,
        edges.left_v,
        edges.top_left_v,
    );
    prediction
}

#[derive(Clone, Copy)]
enum PlanePredictionMode {
    Dc,
    Vertical,
    Horizontal,
    TrueMotion,
}

impl From<Intra16Mode> for PlanePredictionMode {
    fn from(mode: Intra16Mode) -> Self {
        match mode {
            Intra16Mode::Dc => Self::Dc,
            Intra16Mode::Vertical => Self::Vertical,
            Intra16Mode::Horizontal => Self::Horizontal,
            Intra16Mode::TrueMotion => Self::TrueMotion,
        }
    }
}

impl From<ChromaMode> for PlanePredictionMode {
    fn from(mode: ChromaMode) -> Self {
        match mode {
            ChromaMode::Dc => Self::Dc,
            ChromaMode::Vertical => Self::Vertical,
            ChromaMode::Horizontal => Self::Horizontal,
            ChromaMode::TrueMotion => Self::TrueMotion,
        }
    }
}

fn predict_plane<const SIZE: usize>(
    output: &mut [u8],
    mode: PlanePredictionMode,
    top: Option<[u8; SIZE]>,
    left: Option<[u8; SIZE]>,
    top_left: u8,
) {
    debug_assert_eq!(output.len(), SIZE * SIZE);
    match mode {
        PlanePredictionMode::Dc => {
            let value = match (top, left) {
                (Some(top), Some(left)) => {
                    let sum = top.into_iter().map(u32::from).sum::<u32>()
                        + left.into_iter().map(u32::from).sum::<u32>();
                    ((sum + SIZE as u32) / (2 * SIZE) as u32) as u8
                }
                (Some(top), None) => {
                    ((top.into_iter().map(u32::from).sum::<u32>() + (SIZE / 2) as u32)
                        / SIZE as u32) as u8
                }
                (None, Some(left)) => {
                    ((left.into_iter().map(u32::from).sum::<u32>() + (SIZE / 2) as u32)
                        / SIZE as u32) as u8
                }
                (None, None) => 128,
            };
            output.fill(value);
        }
        PlanePredictionMode::Vertical => {
            let top = top.unwrap_or([127; SIZE]);
            for row in output.chunks_exact_mut(SIZE) {
                row.copy_from_slice(&top);
            }
        }
        PlanePredictionMode::Horizontal => {
            let left = left.unwrap_or([129; SIZE]);
            for (row, &value) in output.chunks_exact_mut(SIZE).zip(left.iter()) {
                row.fill(value);
            }
        }
        PlanePredictionMode::TrueMotion => {
            let top_left = match (top, left) {
                (None, _) => 127,
                (Some(_), None) => 129,
                (Some(_), Some(_)) => top_left,
            };
            let top = top.unwrap_or([127; SIZE]);
            let left = left.unwrap_or([129; SIZE]);
            for (row, &left_value) in output.chunks_exact_mut(SIZE).zip(left.iter()) {
                for (sample, &top_value) in row.iter_mut().zip(top.iter()) {
                    *sample = (i32::from(left_value) + i32::from(top_value) - i32::from(top_left))
                        .clamp(0, 255) as u8;
                }
            }
        }
    }
}

/// Predicts one VP8 B_PRED luma 4×4 block from reconstructed neighbours.
/// `top` supplies the four direct and four top-right samples.
#[must_use]
pub fn predict_intra4_block(
    mode: Intra4Mode,
    top_left: u8,
    top: [u8; 8],
    left: [u8; 4],
) -> [u8; 16] {
    let mut out = [0_u8; 16];
    let set = |out: &mut [u8; 16], x: usize, y: usize, value: u8| out[y * 4 + x] = value;
    let a2 = |a: u8, b: u8| ((u16::from(a) + u16::from(b) + 1) >> 1) as u8;
    let a3 =
        |a: u8, b: u8, c: u8| ((u16::from(a) + 2 * u16::from(b) + u16::from(c) + 2) >> 2) as u8;
    match mode {
        Intra4Mode::Dc => {
            let value = (top[..4]
                .iter()
                .chain(left.iter())
                .map(|&value| u16::from(value))
                .sum::<u16>()
                + 4)
                >> 3;
            out.fill(value as u8);
        }
        Intra4Mode::TrueMotion => {
            for (y, &left_value) in left.iter().enumerate() {
                for (x, &top_value) in top[..4].iter().enumerate() {
                    set(
                        &mut out,
                        x,
                        y,
                        (i32::from(left_value) + i32::from(top_value) - i32::from(top_left))
                            .clamp(0, 255) as u8,
                    );
                }
            }
        }
        Intra4Mode::Vertical => {
            let row = [
                a3(top_left, top[0], top[1]),
                a3(top[0], top[1], top[2]),
                a3(top[1], top[2], top[3]),
                a3(top[2], top[3], top[4]),
            ];
            for y in 0..4 {
                out[y * 4..y * 4 + 4].copy_from_slice(&row);
            }
        }
        Intra4Mode::Horizontal => {
            let rows = [
                a3(top_left, left[0], left[1]),
                a3(left[0], left[1], left[2]),
                a3(left[1], left[2], left[3]),
                a3(left[2], left[3], left[3]),
            ];
            for (y, value) in rows.into_iter().enumerate() {
                out[y * 4..y * 4 + 4].fill(value);
            }
        }
        Intra4Mode::DiagonalDownRight => {
            set(&mut out, 0, 3, a3(left[1], left[2], left[3]));
            for (x, y) in [(1, 3), (0, 2)] {
                set(&mut out, x, y, a3(left[0], left[1], left[2]));
            }
            for (x, y) in [(2, 3), (1, 2), (0, 1)] {
                set(&mut out, x, y, a3(top_left, left[0], left[1]));
            }
            for (x, y) in [(3, 3), (2, 2), (1, 1), (0, 0)] {
                set(&mut out, x, y, a3(top[0], top_left, left[0]));
            }
            for (x, y) in [(3, 2), (2, 1), (1, 0)] {
                set(&mut out, x, y, a3(top[1], top[0], top_left));
            }
            for (x, y) in [(3, 1), (2, 0)] {
                set(&mut out, x, y, a3(top[2], top[1], top[0]));
            }
            set(&mut out, 3, 0, a3(top[3], top[2], top[1]));
        }
        Intra4Mode::DiagonalDownLeft => {
            set(&mut out, 0, 0, a3(top[0], top[1], top[2]));
            for (x, y) in [(1, 0), (0, 1)] {
                set(&mut out, x, y, a3(top[1], top[2], top[3]));
            }
            for (x, y) in [(2, 0), (1, 1), (0, 2)] {
                set(&mut out, x, y, a3(top[2], top[3], top[4]));
            }
            for (x, y) in [(3, 0), (2, 1), (1, 2), (0, 3)] {
                set(&mut out, x, y, a3(top[3], top[4], top[5]));
            }
            for (x, y) in [(3, 1), (2, 2), (1, 3)] {
                set(&mut out, x, y, a3(top[4], top[5], top[6]));
            }
            for (x, y) in [(3, 2), (2, 3)] {
                set(&mut out, x, y, a3(top[5], top[6], top[7]));
            }
            set(&mut out, 3, 3, a3(top[6], top[7], top[7]));
        }
        Intra4Mode::VerticalRight => {
            for (x, value) in [
                a2(top_left, top[0]),
                a2(top[0], top[1]),
                a2(top[1], top[2]),
                a2(top[2], top[3]),
            ]
            .into_iter()
            .enumerate()
            {
                set(&mut out, x, 0, value);
            }
            set(&mut out, 0, 3, a3(left[2], left[1], left[0]));
            set(&mut out, 0, 2, a3(left[1], left[0], top_left));
            for (x, y) in [(0, 1), (1, 3)] {
                set(&mut out, x, y, a3(left[0], top_left, top[0]));
            }
            for (x, y) in [(1, 1), (2, 3)] {
                set(&mut out, x, y, a3(top_left, top[0], top[1]));
            }
            for (x, y) in [(2, 1), (3, 3)] {
                set(&mut out, x, y, a3(top[0], top[1], top[2]));
            }
            set(&mut out, 3, 1, a3(top[1], top[2], top[3]));
            for (x, y, value) in [
                (1, 2, a2(top_left, top[0])),
                (2, 2, a2(top[0], top[1])),
                (3, 2, a2(top[1], top[2])),
            ] {
                set(&mut out, x, y, value);
            }
        }
        Intra4Mode::VerticalLeft => {
            for (x, value) in [
                a2(top[0], top[1]),
                a2(top[1], top[2]),
                a2(top[2], top[3]),
                a2(top[3], top[4]),
            ]
            .into_iter()
            .enumerate()
            {
                set(&mut out, x, 0, value);
            }
            for (x, y, value) in [
                (0, 2, a2(top[1], top[2])),
                (1, 2, a2(top[2], top[3])),
                (2, 2, a2(top[3], top[4])),
            ] {
                set(&mut out, x, y, value);
            }
            set(&mut out, 0, 1, a3(top[0], top[1], top[2]));
            for (x, y) in [(1, 1), (0, 3)] {
                set(&mut out, x, y, a3(top[1], top[2], top[3]));
            }
            for (x, y) in [(2, 1), (1, 3)] {
                set(&mut out, x, y, a3(top[2], top[3], top[4]));
            }
            for (x, y) in [(3, 1), (2, 3)] {
                set(&mut out, x, y, a3(top[3], top[4], top[5]));
            }
            set(&mut out, 3, 2, a3(top[4], top[5], top[6]));
            set(&mut out, 3, 3, a3(top[5], top[6], top[7]));
        }
        Intra4Mode::HorizontalUp => {
            set(&mut out, 0, 0, a2(left[0], left[1]));
            for (x, y) in [(2, 0), (0, 1)] {
                set(&mut out, x, y, a2(left[1], left[2]));
            }
            for (x, y) in [(2, 1), (0, 2)] {
                set(&mut out, x, y, a2(left[2], left[3]));
            }
            set(&mut out, 1, 0, a3(left[0], left[1], left[2]));
            for (x, y) in [(3, 0), (1, 1)] {
                set(&mut out, x, y, a3(left[1], left[2], left[3]));
            }
            for (x, y) in [(3, 1), (1, 2)] {
                set(&mut out, x, y, a3(left[2], left[3], left[3]));
            }
            for (x, y) in [(3, 2), (2, 2), (0, 3), (1, 3), (2, 3), (3, 3)] {
                set(&mut out, x, y, left[3]);
            }
        }
        Intra4Mode::HorizontalDown => {
            for (x, y) in [(0, 0), (2, 1)] {
                set(&mut out, x, y, a2(left[0], top_left));
            }
            for (x, y) in [(0, 1), (2, 2)] {
                set(&mut out, x, y, a2(left[1], left[0]));
            }
            for (x, y) in [(0, 2), (2, 3)] {
                set(&mut out, x, y, a2(left[2], left[1]));
            }
            set(&mut out, 0, 3, a2(left[3], left[2]));
            set(&mut out, 3, 0, a3(top[0], top[1], top[2]));
            set(&mut out, 2, 0, a3(top_left, top[0], top[1]));
            for (x, y) in [(1, 0), (3, 1)] {
                set(&mut out, x, y, a3(left[0], top_left, top[0]));
            }
            for (x, y) in [(1, 1), (3, 2)] {
                set(&mut out, x, y, a3(left[1], left[0], top_left));
            }
            for (x, y) in [(1, 2), (3, 3)] {
                set(&mut out, x, y, a3(left[2], left[1], left[0]));
            }
            set(&mut out, 1, 3, a3(left[3], left[2], left[1]));
        }
    }
    out
}
