#![forbid(unsafe_code)]
//! Scalar inverse color transforms used by the WebP lossless (VP8L) decoder.
//!
//! The VP8L color transform is parameterized per rectangular block.  Its three
//! signed, eight-bit multipliers are conventionally carried by a transform
//! image's red, blue, and alpha channels respectively.  The transform image's
//! green channel is unused.

use crate::vp8l::transforms::predictor::Rgba;
use crate::vp8l::transforms::predictor::RgbaImage;
#[cfg(test)]
use crate::vp8l::transforms::predictor::TransformError;
use core::fmt;

/// The number of bits used by the VP8L color-transform block-size field.
pub const COLOR_TRANSFORM_BITS_FIELD_BITS: u8 = 3;
/// The greatest block-size exponent representable in a VP8L color transform.
pub const MAX_COLOR_TRANSFORM_BITS: u8 = (1 << COLOR_TRANSFORM_BITS_FIELD_BITS) - 1;

/// The three signed coefficients of a VP8L color-transform block.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ColorTransformMultipliers {
    /// Coefficient applied to green while reconstructing red.
    pub green_to_red: i8,
    /// Coefficient applied to green while reconstructing blue.
    pub green_to_blue: i8,
    /// Coefficient applied to reconstructed red while reconstructing blue.
    pub red_to_blue: i8,
}

impl ColorTransformMultipliers {
    #[must_use]
    pub const fn new(green_to_red: i8, green_to_blue: i8, red_to_blue: i8) -> Self {
        Self {
            green_to_red,
            green_to_blue,
            red_to_blue,
        }
    }

    /// Decodes coefficients from one pixel of VP8L's transform subimage.
    ///
    /// VP8L stores green-to-red in red, green-to-blue in blue, and red-to-blue
    /// in alpha. Each byte is interpreted as a two's-complement `i8`.
    #[must_use]
    pub const fn from_transform_pixel(pixel: Rgba) -> Self {
        Self::new(pixel.red as i8, pixel.blue as i8, pixel.alpha as i8)
    }
}

/// A row-major table of color-transform coefficients for equally sized blocks.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ColorTransform {
    bits: u8,
    width_blocks: u32,
    height_blocks: u32,
    multipliers: Vec<ColorTransformMultipliers>,
}

impl ColorTransform {
    /// Creates a transform with one coefficient triple per transform-image pixel.
    pub fn new(
        bits: u8,
        width_blocks: u32,
        height_blocks: u32,
        multipliers: Vec<ColorTransformMultipliers>,
    ) -> Result<Self, ColorTransformError> {
        if bits > MAX_COLOR_TRANSFORM_BITS {
            return Err(ColorTransformError::InvalidBlockBits(bits));
        }
        let expected = pixel_len(width_blocks, height_blocks)?;
        if multipliers.len() != expected {
            return Err(ColorTransformError::InvalidMultiplierCount {
                width_blocks,
                height_blocks,
                actual: multipliers.len(),
            });
        }
        Ok(Self {
            bits,
            width_blocks,
            height_blocks,
            multipliers,
        })
    }

    #[must_use]
    pub const fn bits(&self) -> u8 {
        self.bits
    }

    #[must_use]
    pub const fn block_dimensions(&self) -> (u32, u32) {
        (self.width_blocks, self.height_blocks)
    }

    /// Returns the multipliers for the block containing image coordinate `(x, y)`.
    pub fn multipliers_at(
        &self,
        x: u32,
        y: u32,
    ) -> Result<ColorTransformMultipliers, ColorTransformError> {
        let block_x = x >> self.bits;
        let block_y = y >> self.bits;
        if block_x >= self.width_blocks || block_y >= self.height_blocks {
            return Err(ColorTransformError::CoordinateOutsideTransform {
                x,
                y,
                width_blocks: self.width_blocks,
                height_blocks: self.height_blocks,
                bits: self.bits,
            });
        }
        let offset = usize::try_from(block_y)
            .ok()
            .and_then(|row| row.checked_mul(usize::try_from(self.width_blocks).ok()?))
            .and_then(|row_start| row_start.checked_add(usize::try_from(block_x).ok()?))
            .expect("validated transform coordinates must have a backing multiplier");
        Ok(self.multipliers[offset])
    }
}

/// Failure to construct or apply a VP8L color transform.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ColorTransformError {
    ImageTooLarge {
        width: u32,
        height: u32,
    },
    InvalidBlockBits(u8),
    InvalidMultiplierCount {
        width_blocks: u32,
        height_blocks: u32,
        actual: usize,
    },
    CoordinateOutsideTransform {
        x: u32,
        y: u32,
        width_blocks: u32,
        height_blocks: u32,
        bits: u8,
    },
}

impl fmt::Display for ColorTransformError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ImageTooLarge { width, height } => {
                write!(
                    f,
                    "image dimensions {width} by {height} do not fit memory indexing"
                )
            }
            Self::InvalidBlockBits(bits) => write!(
                f,
                "VP8L color-transform block bits {bits} exceed {MAX_COLOR_TRANSFORM_BITS}"
            ),
            Self::InvalidMultiplierCount {
                width_blocks,
                height_blocks,
                actual,
            } => write!(
                f,
                "color-transform table has {actual} entries; expected {width_blocks} by {height_blocks}"
            ),
            Self::CoordinateOutsideTransform {
                x,
                y,
                width_blocks,
                height_blocks,
                bits,
            } => write!(
                f,
                "coordinate ({x}, {y}) is outside {width_blocks} by {height_blocks} color-transform blocks of 2^{bits} pixels"
            ),
        }
    }
}

impl std::error::Error for ColorTransformError {}

/// Returns VP8L's signed, fixed-point color-transform delta.
///
/// This is an arithmetic right shift, not integer division: negative products
/// round down toward negative infinity as the VP8L bitstream specifies.
#[must_use]
pub const fn color_transform_delta(channel: u8, multiplier: i8) -> i16 {
    ((channel as i16) * (multiplier as i16)) >> 5
}

/// Applies the inverse VP8L color transform to one pixel.
///
/// Intermediate channel values deliberately remain signed and unbounded. The
/// final conversion to bytes is modulo 256, and blue uses the reconstructed
/// red value, exactly as required by VP8L.
#[must_use]
pub const fn inverse_color_transform_pixel(
    pixel: Rgba,
    multipliers: ColorTransformMultipliers,
) -> Rgba {
    // `red` can temporarily range beyond a byte. Keep the final blue product
    // in `i32`: 1,267 * 127 is a valid VP8L intermediate value.
    let red =
        (pixel.red as i32) + (color_transform_delta(pixel.green, multipliers.green_to_red) as i32);
    let blue = (pixel.blue as i32)
        + (color_transform_delta(pixel.green, multipliers.green_to_blue) as i32)
        + ((red * (multipliers.red_to_blue as i32)) >> 5);
    Rgba::new(red as u8, pixel.green, blue as u8, pixel.alpha)
}

/// Applies an inverse VP8L color transform to every pixel in place.
pub fn inverse_color_transform(
    image: &mut RgbaImage,
    transform: &ColorTransform,
) -> Result<(), ColorTransformError> {
    let width = image.width();
    let height = image.height();
    for y in 0..height {
        for x in 0..width {
            let multiplier = transform.multipliers_at(x, y)?;
            let offset = usize::try_from(y)
                .ok()
                .and_then(|row| row.checked_mul(usize::try_from(width).ok()?))
                .and_then(|row_start| row_start.checked_add(usize::try_from(x).ok()?))
                .expect("validated image coordinates must have a backing pixel");
            let pixel = image.pixels()[offset];
            image.pixels_mut()[offset] = inverse_color_transform_pixel(pixel, multiplier);
        }
    }
    Ok(())
}

fn pixel_len(width: u32, height: u32) -> Result<usize, ColorTransformError> {
    let pixels = u64::from(width)
        .checked_mul(u64::from(height))
        .ok_or(ColorTransformError::ImageTooLarge { width, height })?;
    usize::try_from(pixels).map_err(|_| ColorTransformError::ImageTooLarge { width, height })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delta_interprets_wire_multipliers_as_signed_and_rounds_down() {
        let cases = [
            (0_u8, 0_i8, 0_i16),
            (255, 1, 7),
            (255, 0x7f, 1_012),
            (255, 0x80_u8 as i8, -1_020),
            (255, 0xff_u8 as i8, -8),
            (1, -1, -1),
            (31, -1, -1),
            (32, -1, -1),
        ];
        for (channel, multiplier, expected) in cases {
            assert_eq!(color_transform_delta(channel, multiplier), expected);
        }
    }

    #[test]
    fn transform_pixel_assigns_the_specified_channels() {
        let multipliers =
            ColorTransformMultipliers::from_transform_pixel(Rgba::new(0x80, 0x42, 0xff, 0x7f));
        assert_eq!(multipliers, ColorTransformMultipliers::new(-128, -1, 127));
    }

    #[test]
    fn inverse_transform_preserves_green_and_alpha_and_wraps_channels() {
        let pixel = Rgba::new(250, 255, 250, 17);
        let result =
            inverse_color_transform_pixel(pixel, ColorTransformMultipliers::new(1, -1, -128));
        assert_eq!(result, Rgba::new(1, 255, 238, 17));
    }

    #[test]
    fn inverse_transform_handles_channel_extrema() {
        let low = inverse_color_transform_pixel(
            Rgba::new(0, 0, 0, 0),
            ColorTransformMultipliers::new(-128, -128, -128),
        );
        let high = inverse_color_transform_pixel(
            Rgba::new(255, 255, 255, 255),
            ColorTransformMultipliers::new(127, 127, 127),
        );
        assert_eq!(low, Rgba::new(0, 0, 0, 0));
        assert_eq!(high, Rgba::new(243, 255, 151, 255));
    }

    #[test]
    fn block_lookup_uses_shifted_coordinates_at_all_boundaries() {
        let table = ColorTransform::new(
            1,
            3,
            2,
            (0_i8..6)
                .map(|coefficient| ColorTransformMultipliers::new(coefficient, 0, 0))
                .collect(),
        )
        .unwrap();
        assert_eq!(table.multipliers_at(0, 0).unwrap().green_to_red, 0);
        assert_eq!(table.multipliers_at(1, 1).unwrap().green_to_red, 0);
        assert_eq!(table.multipliers_at(2, 0).unwrap().green_to_red, 1);
        assert_eq!(table.multipliers_at(3, 1).unwrap().green_to_red, 1);
        assert_eq!(table.multipliers_at(4, 0).unwrap().green_to_red, 2);
        assert_eq!(table.multipliers_at(0, 2).unwrap().green_to_red, 3);
        assert_eq!(table.multipliers_at(4, 2).unwrap().green_to_red, 5);
        assert!(matches!(
            table.multipliers_at(6, 0),
            Err(ColorTransformError::CoordinateOutsideTransform { .. })
        ));
    }

    #[test]
    fn application_selects_a_multiplier_for_each_partial_edge_block() {
        let transform = ColorTransform::new(
            1,
            3,
            2,
            (0_i8..6)
                .map(|coefficient| ColorTransformMultipliers::new(coefficient, 0, 0))
                .collect(),
        )
        .unwrap();
        let mut image = RgbaImage::new(5, 3, vec![Rgba::new(0, 32, 0, 9); 15]).unwrap();
        inverse_color_transform(&mut image, &transform).unwrap();
        assert_eq!(
            image.pixels(),
            &[
                Rgba::new(0, 32, 0, 9),
                Rgba::new(0, 32, 0, 9),
                Rgba::new(1, 32, 0, 9),
                Rgba::new(1, 32, 0, 9),
                Rgba::new(2, 32, 0, 9),
                Rgba::new(0, 32, 0, 9),
                Rgba::new(0, 32, 0, 9),
                Rgba::new(1, 32, 0, 9),
                Rgba::new(1, 32, 0, 9),
                Rgba::new(2, 32, 0, 9),
                Rgba::new(3, 32, 0, 9),
                Rgba::new(3, 32, 0, 9),
                Rgba::new(4, 32, 0, 9),
                Rgba::new(4, 32, 0, 9),
                Rgba::new(5, 32, 0, 9),
            ]
        );
    }

    #[test]
    fn image_and_transform_accessors_report_the_validated_configuration() {
        let mut image = RgbaImage::new(
            2,
            3,
            (0..6)
                .map(|value| Rgba::new(value, value + 10, value + 20, value + 30))
                .collect(),
        )
        .unwrap();
        assert_eq!(image.width(), 2);
        assert_eq!(image.height(), 3);
        image.pixels_mut()[4] = Rgba::new(90, 91, 92, 93);
        assert_eq!(image.pixels()[4], Rgba::new(90, 91, 92, 93));

        let transform =
            ColorTransform::new(2, 2, 3, vec![ColorTransformMultipliers::default(); 6]).unwrap();
        assert_eq!(transform.bits(), 2);
        assert_eq!(transform.block_dimensions(), (2, 3));
    }

    #[test]
    fn construction_rejects_unrepresentable_tables() {
        assert!(matches!(
            ColorTransform::new(8, 1, 1, vec![ColorTransformMultipliers::default()]),
            Err(ColorTransformError::InvalidBlockBits(8))
        ));
        assert!(matches!(
            ColorTransform::new(0, 2, 1, vec![ColorTransformMultipliers::default()]),
            Err(ColorTransformError::InvalidMultiplierCount { .. })
        ));
        assert!(matches!(
            RgbaImage::new(2, 2, vec![Rgba::default(); 3]),
            Err(TransformError::InvalidBufferLength { .. })
        ));
        assert_eq!(
            ColorTransformError::InvalidBlockBits(8).to_string(),
            "VP8L color-transform block bits 8 exceed 7"
        );
    }
}
