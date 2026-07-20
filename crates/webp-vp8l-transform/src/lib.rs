#![forbid(unsafe_code)]
//! Scalar inverse transforms used by the WebP lossless (VP8L) decoder.
//!
//! Pixels are represented in RGBA order. Arithmetic which combines a decoded
//! predictor and a VP8L residual is intentionally wrapping modulo 256, as
//! required by the bitstream specification.

use core::fmt;

/// A straight, eight-bit-per-channel RGBA pixel.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Rgba {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    pub alpha: u8,
}

impl Rgba {
    /// Opaque black, used for the top-left predictor boundary.
    pub const OPAQUE_BLACK: Self = Self {
        red: 0,
        green: 0,
        blue: 0,
        alpha: u8::MAX,
    };

    #[must_use]
    pub const fn new(red: u8, green: u8, blue: u8, alpha: u8) -> Self {
        Self {
            red,
            green,
            blue,
            alpha,
        }
    }

    fn wrapping_add(self, other: Self) -> Self {
        Self::new(
            self.red.wrapping_add(other.red),
            self.green.wrapping_add(other.green),
            self.blue.wrapping_add(other.blue),
            self.alpha.wrapping_add(other.alpha),
        )
    }
}

/// A validated, row-major RGBA image buffer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RgbaImage {
    width: u32,
    height: u32,
    pixels: Vec<Rgba>,
}

impl RgbaImage {
    /// Creates an image when `pixels` has exactly `width * height` entries.
    pub fn new(width: u32, height: u32, pixels: Vec<Rgba>) -> Result<Self, TransformError> {
        let expected_len = pixel_len(width, height)?;
        if pixels.len() != expected_len {
            return Err(TransformError::InvalidBufferLength {
                width,
                height,
                actual: pixels.len(),
            });
        }
        Ok(Self {
            width,
            height,
            pixels,
        })
    }

    #[must_use]
    pub const fn width(&self) -> u32 {
        self.width
    }

    #[must_use]
    pub const fn height(&self) -> u32 {
        self.height
    }

    #[must_use]
    pub fn pixels(&self) -> &[Rgba] {
        &self.pixels
    }

    #[must_use]
    pub fn pixels_mut(&mut self) -> &mut [Rgba] {
        &mut self.pixels
    }

    #[must_use]
    pub fn get(&self, x: u32, y: u32) -> Option<Rgba> {
        self.offset(x, y).map(|offset| self.pixels[offset])
    }

    fn offset(&self, x: u32, y: u32) -> Option<usize> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let width = usize::try_from(self.width).ok()?;
        let x = usize::try_from(x).ok()?;
        let y = usize::try_from(y).ok()?;
        y.checked_mul(width)?.checked_add(x)
    }

    fn offset_in_bounds(&self, x: u32, y: u32) -> usize {
        // Every caller has proved the coordinate is in bounds. Construction
        // has also proved that the row-major offset fits the backing buffer.
        self.offset(x, y)
            .expect("validated image coordinate must have a backing pixel")
    }
}

/// One of VP8L's fourteen spatial predictor modes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum PredictorMode {
    OpaqueBlack = 0,
    Left = 1,
    Top = 2,
    TopRight = 3,
    TopLeft = 4,
    AverageLeftTopRightTop = 5,
    AverageLeftTopLeft = 6,
    AverageLeftTop = 7,
    AverageTopLeftTop = 8,
    AverageTopTopRight = 9,
    AverageLeftTopLeftTopTopRight = 10,
    Select = 11,
    ClampAddSubtractFull = 12,
    ClampAddSubtractHalf = 13,
}

impl TryFrom<u8> for PredictorMode {
    type Error = TransformError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::OpaqueBlack),
            1 => Ok(Self::Left),
            2 => Ok(Self::Top),
            3 => Ok(Self::TopRight),
            4 => Ok(Self::TopLeft),
            5 => Ok(Self::AverageLeftTopRightTop),
            6 => Ok(Self::AverageLeftTopLeft),
            7 => Ok(Self::AverageLeftTop),
            8 => Ok(Self::AverageTopLeftTop),
            9 => Ok(Self::AverageTopTopRight),
            10 => Ok(Self::AverageLeftTopLeftTopTopRight),
            11 => Ok(Self::Select),
            12 => Ok(Self::ClampAddSubtractFull),
            13 => Ok(Self::ClampAddSubtractHalf),
            _ => Err(TransformError::InvalidPredictorMode(value)),
        }
    }
}

/// Failure to construct or apply an inverse VP8L transform.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TransformError {
    InvalidBufferLength {
        width: u32,
        height: u32,
        actual: usize,
    },
    ImageTooLarge {
        width: u32,
        height: u32,
    },
    InvalidPredictorMode(u8),
    InvalidModeCount {
        expected: usize,
        actual: usize,
    },
    CoordinateOutOfBounds {
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    },
}

impl fmt::Display for TransformError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidBufferLength {
                width,
                height,
                actual,
            } => write!(
                f,
                "RGBA buffer has {actual} pixels; expected {width} by {height} pixels"
            ),
            Self::ImageTooLarge { width, height } => {
                write!(
                    f,
                    "image dimensions {width} by {height} do not fit memory indexing"
                )
            }
            Self::InvalidPredictorMode(mode) => write!(f, "invalid VP8L predictor mode {mode}"),
            Self::InvalidModeCount { expected, actual } => write!(
                f,
                "predictor mode buffer has {actual} entries; expected {expected}"
            ),
            Self::CoordinateOutOfBounds {
                x,
                y,
                width,
                height,
            } => write!(f, "coordinate ({x}, {y}) is outside {width} by {height}"),
        }
    }
}

impl std::error::Error for TransformError {}

/// Reverses VP8L's subtract-green transform in place.
///
/// The red and blue residual channels add green modulo 256; alpha and green
/// themselves are unchanged.
pub fn inverse_subtract_green(image: &mut RgbaImage) {
    for pixel in image.pixels_mut() {
        pixel.red = pixel.red.wrapping_add(pixel.green);
        pixel.blue = pixel.blue.wrapping_add(pixel.green);
    }
}

/// Computes a predictor using already reconstructed neighboring pixels.
///
/// At the top-left pixel this returns opaque black. On the remaining top row
/// it returns the left pixel, and on the remaining left column it returns the
/// top pixel, regardless of `mode`. On the right edge, `TopRight` is the
/// leftmost pixel of the preceding row, as prescribed by VP8L.
pub fn prediction_at(
    image: &RgbaImage,
    x: u32,
    y: u32,
    mode: PredictorMode,
) -> Result<Rgba, TransformError> {
    if x >= image.width || y >= image.height {
        return Err(TransformError::CoordinateOutOfBounds {
            x,
            y,
            width: image.width,
            height: image.height,
        });
    }
    if x == 0 && y == 0 {
        return Ok(Rgba::OPAQUE_BLACK);
    }
    if y == 0 {
        return Ok(image.pixels[image.offset_in_bounds(x - 1, y)]);
    }
    if x == 0 {
        return Ok(image.pixels[image.offset_in_bounds(x, y - 1)]);
    }

    let left = image.pixels[image.offset_in_bounds(x - 1, y)];
    let top = image.pixels[image.offset_in_bounds(x, y - 1)];
    let top_left = image.pixels[image.offset_in_bounds(x - 1, y - 1)];
    let top_right_x = if x + 1 == image.width { 0 } else { x + 1 };
    let top_right = image.pixels[image.offset_in_bounds(top_right_x, y - 1)];
    Ok(predict(mode, left, top, top_left, top_right))
}

/// Computes one of the fourteen VP8L predictors from its four neighbors.
#[must_use]
pub fn predict(
    mode: PredictorMode,
    left: Rgba,
    top: Rgba,
    top_left: Rgba,
    top_right: Rgba,
) -> Rgba {
    match mode {
        PredictorMode::OpaqueBlack => Rgba::OPAQUE_BLACK,
        PredictorMode::Left => left,
        PredictorMode::Top => top,
        PredictorMode::TopRight => top_right,
        PredictorMode::TopLeft => top_left,
        PredictorMode::AverageLeftTopRightTop => average(average(left, top_right), top),
        PredictorMode::AverageLeftTopLeft => average(left, top_left),
        PredictorMode::AverageLeftTop => average(left, top),
        PredictorMode::AverageTopLeftTop => average(top_left, top),
        PredictorMode::AverageTopTopRight => average(top, top_right),
        PredictorMode::AverageLeftTopLeftTopTopRight => {
            average(average(left, top_left), average(top, top_right))
        }
        PredictorMode::Select => select(left, top, top_left),
        PredictorMode::ClampAddSubtractFull => clamp_add_subtract_full(left, top, top_left),
        PredictorMode::ClampAddSubtractHalf => {
            clamp_add_subtract_half(average(left, top), top_left)
        }
    }
}

/// Reverses the predictor transform using one mode per image pixel.
///
/// Modes for boundary pixels are accepted but ignored according to VP8L's
/// fixed border rules. The buffer is traversed in scan-line order.
pub fn inverse_predictor(
    image: &mut RgbaImage,
    modes: &[PredictorMode],
) -> Result<(), TransformError> {
    if modes.len() != image.pixels.len() {
        return Err(TransformError::InvalidModeCount {
            expected: image.pixels.len(),
            actual: modes.len(),
        });
    }
    let width = usize::try_from(image.width).map_err(|_| TransformError::ImageTooLarge {
        width: image.width,
        height: image.height,
    })?;
    inverse_predictor_with(image, |x, y| {
        modes[usize::try_from(y).expect("u32 fits usize") * width
            + usize::try_from(x).expect("u32 fits usize")]
    });
    Ok(())
}

/// Reverses the predictor transform with a caller-supplied mode lookup.
pub fn inverse_predictor_with<F>(image: &mut RgbaImage, mut mode_at: F)
where
    F: FnMut(u32, u32) -> PredictorMode,
{
    for y in 0..image.height {
        for x in 0..image.width {
            let mode = mode_at(x, y);
            let prediction = prediction_at(image, x, y, mode)
                .expect("coordinates generated from a validated image are in bounds");
            let offset = image.offset_in_bounds(x, y);
            image.pixels[offset] = image.pixels[offset].wrapping_add(prediction);
        }
    }
}

fn pixel_len(width: u32, height: u32) -> Result<usize, TransformError> {
    let original_width = width;
    let original_height = height;
    let width = usize::try_from(width).map_err(|_| TransformError::ImageTooLarge {
        width: original_width,
        height: original_height,
    })?;
    let height = usize::try_from(height).map_err(|_| TransformError::ImageTooLarge {
        width: original_width,
        height: original_height,
    })?;
    width
        .checked_mul(height)
        .ok_or(TransformError::ImageTooLarge {
            width: original_width,
            height: original_height,
        })
}

fn average(a: Rgba, b: Rgba) -> Rgba {
    Rgba::new(
        average_channel(a.red, b.red),
        average_channel(a.green, b.green),
        average_channel(a.blue, b.blue),
        average_channel(a.alpha, b.alpha),
    )
}

fn average_channel(a: u8, b: u8) -> u8 {
    ((u16::from(a) + u16::from(b)) / 2) as u8
}

fn select(left: Rgba, top: Rgba, top_left: Rgba) -> Rgba {
    let estimate = [
        i16::from(left.red) + i16::from(top.red) - i16::from(top_left.red),
        i16::from(left.green) + i16::from(top.green) - i16::from(top_left.green),
        i16::from(left.blue) + i16::from(top.blue) - i16::from(top_left.blue),
        i16::from(left.alpha) + i16::from(top.alpha) - i16::from(top_left.alpha),
    ];
    let left_channels = [left.red, left.green, left.blue, left.alpha];
    let top_channels = [top.red, top.green, top.blue, top.alpha];
    let left_distance: i16 = estimate
        .iter()
        .zip(left_channels)
        .map(|(estimate, channel)| (estimate - i16::from(channel)).abs())
        .sum();
    let top_distance: i16 = estimate
        .iter()
        .zip(top_channels)
        .map(|(estimate, channel)| (estimate - i16::from(channel)).abs())
        .sum();
    if left_distance < top_distance {
        left
    } else {
        top
    }
}

fn clamp_add_subtract_full(a: Rgba, b: Rgba, c: Rgba) -> Rgba {
    Rgba::new(
        clamp_channel(i16::from(a.red) + i16::from(b.red) - i16::from(c.red)),
        clamp_channel(i16::from(a.green) + i16::from(b.green) - i16::from(c.green)),
        clamp_channel(i16::from(a.blue) + i16::from(b.blue) - i16::from(c.blue)),
        clamp_channel(i16::from(a.alpha) + i16::from(b.alpha) - i16::from(c.alpha)),
    )
}

fn clamp_add_subtract_half(a: Rgba, b: Rgba) -> Rgba {
    Rgba::new(
        clamp_channel(i16::from(a.red) + (i16::from(a.red) - i16::from(b.red)) / 2),
        clamp_channel(i16::from(a.green) + (i16::from(a.green) - i16::from(b.green)) / 2),
        clamp_channel(i16::from(a.blue) + (i16::from(a.blue) - i16::from(b.blue)) / 2),
        clamp_channel(i16::from(a.alpha) + (i16::from(a.alpha) - i16::from(b.alpha)) / 2),
    )
}

fn clamp_channel(value: i16) -> u8 {
    value.clamp(0, i16::from(u8::MAX)) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    const LEFT: Rgba = Rgba::new(20, 110, 250, 5);
    const TOP: Rgba = Rgba::new(200, 80, 30, 254);
    const TOP_LEFT: Rgba = Rgba::new(100, 90, 100, 125);
    const TOP_RIGHT: Rgba = Rgba::new(18, 210, 60, 40);

    #[test]
    fn every_predictor_mode_matches_the_specification_table() {
        let cases = [
            (PredictorMode::OpaqueBlack, Rgba::new(0, 0, 0, 255)),
            (PredictorMode::Left, LEFT),
            (PredictorMode::Top, TOP),
            (PredictorMode::TopRight, TOP_RIGHT),
            (PredictorMode::TopLeft, TOP_LEFT),
            (
                PredictorMode::AverageLeftTopRightTop,
                Rgba::new(109, 120, 92, 138),
            ),
            (
                PredictorMode::AverageLeftTopLeft,
                Rgba::new(60, 100, 175, 65),
            ),
            (PredictorMode::AverageLeftTop, Rgba::new(110, 95, 140, 129)),
            (
                PredictorMode::AverageTopLeftTop,
                Rgba::new(150, 85, 65, 189),
            ),
            (
                PredictorMode::AverageTopTopRight,
                Rgba::new(109, 145, 45, 147),
            ),
            (
                PredictorMode::AverageLeftTopLeftTopTopRight,
                Rgba::new(84, 122, 110, 106),
            ),
            (PredictorMode::Select, LEFT),
            (
                PredictorMode::ClampAddSubtractFull,
                Rgba::new(120, 100, 180, 134),
            ),
            (
                PredictorMode::ClampAddSubtractHalf,
                Rgba::new(115, 97, 160, 131),
            ),
        ];
        for (mode, expected) in cases {
            assert_eq!(
                predict(mode, LEFT, TOP, TOP_LEFT, TOP_RIGHT),
                expected,
                "{mode:?}"
            );
        }
    }

    #[test]
    fn predictor_borders_override_every_mode_and_wrap_top_right() {
        let pixels = vec![
            Rgba::new(1, 2, 3, 4),
            Rgba::new(5, 6, 7, 8),
            Rgba::new(9, 10, 11, 12),
            Rgba::new(13, 14, 15, 16),
            Rgba::new(17, 18, 19, 20),
            Rgba::new(21, 22, 23, 24),
        ];
        let image = RgbaImage::new(3, 2, pixels).unwrap();
        for mode_value in 0..14 {
            let mode = PredictorMode::try_from(mode_value).unwrap();
            assert_eq!(
                prediction_at(&image, 0, 0, mode).unwrap(),
                Rgba::OPAQUE_BLACK
            );
            assert_eq!(
                prediction_at(&image, 2, 0, mode).unwrap(),
                Rgba::new(5, 6, 7, 8)
            );
            assert_eq!(
                prediction_at(&image, 0, 1, mode).unwrap(),
                Rgba::new(1, 2, 3, 4)
            );
        }
        assert_eq!(
            prediction_at(&image, 2, 1, PredictorMode::TopRight).unwrap(),
            Rgba::new(1, 2, 3, 4),
        );
    }

    #[test]
    fn inverse_predictor_uses_reconstructed_neighbors_and_wrapping_addition() {
        let residuals = vec![
            Rgba::new(1, 2, 3, 4),
            Rgba::new(4, 5, 6, 7),
            Rgba::new(8, 9, 10, 11),
            Rgba::new(12, 13, 14, 15),
            Rgba::new(255, 254, 253, 252),
            Rgba::new(16, 17, 18, 19),
        ];
        let mut image = RgbaImage::new(3, 2, residuals).unwrap();
        let modes = [
            PredictorMode::TopRight,
            PredictorMode::Top,
            PredictorMode::OpaqueBlack,
            PredictorMode::Left,
            PredictorMode::AverageLeftTop,
            PredictorMode::TopRight,
        ];
        inverse_predictor(&mut image, &modes).unwrap();
        assert_eq!(
            image.pixels(),
            [
                Rgba::new(1, 2, 3, 3),
                Rgba::new(5, 7, 9, 10),
                Rgba::new(13, 16, 19, 21),
                Rgba::new(13, 15, 17, 18),
                Rgba::new(8, 9, 10, 10),
                Rgba::new(17, 19, 21, 22),
            ]
        );
    }

    #[test]
    fn subtract_green_and_clamped_modes_handle_channel_extremes() {
        let mut image = RgbaImage::new(1, 1, vec![Rgba::new(0, 255, 1, 77)]).unwrap();
        inverse_subtract_green(&mut image);
        assert_eq!(image.pixels(), [Rgba::new(255, 255, 0, 77)]);

        let minimum = Rgba::new(0, 0, 0, 0);
        let maximum = Rgba::new(255, 255, 255, 255);
        assert_eq!(
            predict(
                PredictorMode::ClampAddSubtractFull,
                maximum,
                maximum,
                minimum,
                minimum
            ),
            maximum
        );
        assert_eq!(
            predict(
                PredictorMode::ClampAddSubtractFull,
                minimum,
                minimum,
                maximum,
                maximum
            ),
            minimum
        );
        assert_eq!(
            predict(
                PredictorMode::ClampAddSubtractHalf,
                maximum,
                maximum,
                minimum,
                minimum
            ),
            maximum
        );
        assert_eq!(
            predict(
                PredictorMode::ClampAddSubtractHalf,
                minimum,
                minimum,
                maximum,
                maximum
            ),
            minimum
        );
    }

    #[test]
    fn invalid_inputs_are_rejected_without_panicking() {
        assert!(matches!(
            RgbaImage::new(2, 2, vec![Rgba::default(); 3]),
            Err(TransformError::InvalidBufferLength { .. })
        ));
        assert_eq!(
            PredictorMode::try_from(14),
            Err(TransformError::InvalidPredictorMode(14))
        );
        let mut image = RgbaImage::new(1, 1, vec![Rgba::default()]).unwrap();
        assert!(matches!(
            inverse_predictor(&mut image, &[]),
            Err(TransformError::InvalidModeCount { .. })
        ));
        assert!(matches!(
            prediction_at(&image, 1, 0, PredictorMode::Left),
            Err(TransformError::CoordinateOutOfBounds { .. })
        ));
    }
}
