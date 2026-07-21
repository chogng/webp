#![forbid(unsafe_code)]
//! Inverse color-indexing for WebP lossless (VP8L) images.
//!
//! A VP8L color-indexing transform replaces an image pixel with a palette
//! index held in the pixel's green channel. Small palettes pack several
//! indices into that channel, least-significant index first. This crate keeps
//! that layout explicit: no native-endian packed-pixel reinterpretation is
//! used, and an invalid palette index reconstructs as transparent black as
//! required by the VP8L specification.

use core::fmt;

use webp_vp8l_transform::{Rgba, RgbaImage};

/// The greatest palette size representable by VP8L color indexing.
pub const MAX_PALETTE_SIZE: usize = 256;

/// Transparent black used when a decoded index is outside the palette.
pub const TRANSPARENT_BLACK: Rgba = Rgba::new(0, 0, 0, 0);

/// A validated VP8L color table whose entries are already delta-decoded.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Palette {
    entries: Box<[Rgba]>,
}

impl Palette {
    /// Creates a palette from fully reconstructed entries.
    pub fn new(entries: Vec<Rgba>) -> Result<Self, ColorIndexError> {
        validate_palette_size(entries.len())?;
        Ok(Self {
            entries: entries.into_boxed_slice(),
        })
    }

    /// Creates a palette from VP8L's delta-coded palette image.
    ///
    /// Entry zero is literal. Every following entry is added channel by
    /// channel to the preceding reconstructed entry modulo 256.
    pub fn from_deltas(mut entries: Vec<Rgba>) -> Result<Self, ColorIndexError> {
        inverse_palette_deltas(&mut entries)?;
        Self::new(entries)
    }

    /// Number of entries in this palette.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the palette has no entries. Valid VP8L palettes are never empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns the reconstructed entry at `index`, if it is in range.
    #[must_use]
    pub fn get(&self, index: usize) -> Option<Rgba> {
        self.entries.get(index).copied()
    }

    /// Returns all reconstructed palette entries.
    #[must_use]
    pub fn entries(&self) -> &[Rgba] {
        &self.entries
    }

    /// Returns the number of packed output pixels represented by one source
    /// pixel and the bit width of each index.
    #[must_use]
    pub fn packing(&self) -> IndexPacking {
        IndexPacking::for_palette_size(self.len())
            .expect("Palette construction validates every supported size")
    }
}

/// The packing selected by a VP8L palette size.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IndexPacking {
    bits_per_index: u8,
    indices_per_pixel: u8,
}

impl IndexPacking {
    /// Selects the VP8L packing for a palette with `palette_size` entries.
    pub fn for_palette_size(palette_size: usize) -> Result<Self, ColorIndexError> {
        validate_palette_size(palette_size)?;
        let bits_per_index = match palette_size {
            1..=2 => 1,
            3..=4 => 2,
            5..=16 => 4,
            17..=MAX_PALETTE_SIZE => 8,
            _ => unreachable!("palette size was validated"),
        };
        Ok(Self {
            bits_per_index,
            indices_per_pixel: u8::BITS as u8 / bits_per_index,
        })
    }

    /// Number of bits occupied by one palette index.
    #[must_use]
    pub const fn bits_per_index(self) -> u8 {
        self.bits_per_index
    }

    /// Number of output pixels packed into one source pixel.
    #[must_use]
    pub const fn indices_per_pixel(self) -> u8 {
        self.indices_per_pixel
    }

    /// Computes the indexed-image width needed to reconstruct `output_width`.
    pub fn packed_width(self, output_width: u32) -> Result<u32, ColorIndexError> {
        let bundle = u32::from(self.indices_per_pixel);
        output_width
            .checked_add(bundle - 1)
            .map(|adjusted| adjusted / bundle)
            .ok_or(ColorIndexError::DimensionOverflow { output_width })
    }
}

/// Failure to validate or invert a VP8L color-indexing transform.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ColorIndexError {
    InvalidPaletteSize {
        actual: usize,
    },
    DimensionOverflow {
        output_width: u32,
    },
    IndexedDimensions {
        expected_width: u32,
        actual_width: u32,
        expected_height: u32,
        actual_height: u32,
    },
    ImageTooLarge {
        width: u32,
        height: u32,
    },
    AllocationFailed {
        width: u32,
        height: u32,
    },
}

impl fmt::Display for ColorIndexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPaletteSize { actual } => write!(
                f,
                "VP8L color-indexing palette has {actual} entries; expected 1..={MAX_PALETTE_SIZE}"
            ),
            Self::DimensionOverflow { output_width } => write!(
                f,
                "output width {output_width} overflows VP8L color-indexing packing"
            ),
            Self::IndexedDimensions {
                expected_width,
                actual_width,
                expected_height,
                actual_height,
            } => write!(
                f,
                "indexed image is {actual_width} by {actual_height}; expected {expected_width} by {expected_height}"
            ),
            Self::ImageTooLarge { width, height } => write!(
                f,
                "image dimensions {width} by {height} do not fit memory indexing"
            ),
            Self::AllocationFailed { width, height } => write!(
                f,
                "cannot allocate a {width} by {height} color-indexed image"
            ),
        }
    }
}

impl std::error::Error for ColorIndexError {}

/// Reconstructs a delta-coded VP8L palette in place.
///
/// The palette has one to 256 entries. All four channels, including alpha and
/// RGB values of transparent entries, use wrapping arithmetic.
pub fn inverse_palette_deltas(entries: &mut [Rgba]) -> Result<(), ColorIndexError> {
    validate_palette_size(entries.len())?;
    for index in 1..entries.len() {
        entries[index] = wrapping_add(entries[index - 1], entries[index]);
    }
    Ok(())
}

/// Reverses a VP8L color-indexing transform.
///
/// `indexed` has the same height as the reconstructed image, but has the
/// packed width returned by [`IndexPacking::packed_width`]. The low bits of
/// each input pixel's green channel contain the leftmost palette index; the
/// next group of bits contains the pixel immediately to its right. Bits not
/// needed at the end of a row are ignored. Red, blue, and alpha of `indexed`
/// are intentionally ignored.
///
/// Palette indices outside the palette yield [`TRANSPARENT_BLACK`], allowing
/// malformed streams to recover with the behavior prescribed by VP8L.
pub fn inverse_color_indexing(
    palette: &Palette,
    output_width: u32,
    indexed: &RgbaImage,
) -> Result<RgbaImage, ColorIndexError> {
    let output_height = indexed.height();
    let packing = palette.packing();
    let expected_width = packing.packed_width(output_width)?;
    if indexed.width() != expected_width {
        return Err(ColorIndexError::IndexedDimensions {
            expected_width,
            actual_width: indexed.width(),
            expected_height: output_height,
            actual_height: indexed.height(),
        });
    }

    let pixel_count = pixel_count(output_width, output_height)?;
    if output_width == 0 {
        return RgbaImage::new(0, output_height, Vec::new()).map_err(|_| {
            ColorIndexError::ImageTooLarge {
                width: output_width,
                height: output_height,
            }
        });
    }
    let mut pixels = Vec::new();
    pixels
        .try_reserve_exact(pixel_count)
        .map_err(|_| ColorIndexError::AllocationFailed {
            width: output_width,
            height: output_height,
        })?;

    let bits = u32::from(packing.bits_per_index());
    let mask = (1_u16 << bits) - 1;
    let packed_width =
        usize::try_from(expected_width).map_err(|_| ColorIndexError::ImageTooLarge {
            width: output_width,
            height: output_height,
        })?;
    let output_width_usize =
        usize::try_from(output_width).map_err(|_| ColorIndexError::ImageTooLarge {
            width: output_width,
            height: output_height,
        })?;

    for row in indexed.pixels().chunks_exact(packed_width) {
        for x in 0..output_width_usize {
            let packed = row[x / usize::from(packing.indices_per_pixel())].green;
            let shift = (x % usize::from(packing.indices_per_pixel())) as u32 * bits;
            let index = usize::from((u16::from(packed) >> shift) & mask);
            pixels.push(palette.get(index).unwrap_or(TRANSPARENT_BLACK));
        }
    }

    RgbaImage::new(output_width, output_height, pixels).map_err(|_| {
        ColorIndexError::ImageTooLarge {
            width: output_width,
            height: output_height,
        }
    })
}

fn validate_palette_size(actual: usize) -> Result<(), ColorIndexError> {
    if !(1..=MAX_PALETTE_SIZE).contains(&actual) {
        return Err(ColorIndexError::InvalidPaletteSize { actual });
    }
    Ok(())
}

fn pixel_count(width: u32, height: u32) -> Result<usize, ColorIndexError> {
    let width_usize =
        usize::try_from(width).map_err(|_| ColorIndexError::ImageTooLarge { width, height })?;
    let height_usize =
        usize::try_from(height).map_err(|_| ColorIndexError::ImageTooLarge { width, height })?;
    width_usize
        .checked_mul(height_usize)
        .ok_or(ColorIndexError::ImageTooLarge { width, height })
}

fn wrapping_add(previous: Rgba, delta: Rgba) -> Rgba {
    Rgba::new(
        previous.red.wrapping_add(delta.red),
        previous.green.wrapping_add(delta.green),
        previous.blue.wrapping_add(delta.blue),
        previous.alpha.wrapping_add(delta.alpha),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn palette(size: usize) -> Palette {
        Palette::new(
            (0..size)
                .map(|index| {
                    let index = index as u8;
                    Rgba::new(
                        index,
                        index.wrapping_add(1),
                        index.wrapping_add(2),
                        255 - index,
                    )
                })
                .collect(),
        )
        .unwrap()
    }

    fn indexed_image(width: u32, height: u32, green: Vec<u8>) -> RgbaImage {
        RgbaImage::new(
            width,
            height,
            green
                .into_iter()
                .map(|green| Rgba::new(0xa5, green, 0x5a, 0x33))
                .collect(),
        )
        .unwrap()
    }

    fn pack_row(indices: &[u8], packing: IndexPacking) -> Vec<u8> {
        let bundle = usize::from(packing.indices_per_pixel());
        let bits = u32::from(packing.bits_per_index());
        indices
            .chunks(bundle)
            .map(|chunk| {
                chunk
                    .iter()
                    .enumerate()
                    .fold(0_u8, |packed, (position, &index)| {
                        packed | (index << (u32::try_from(position).unwrap() * bits))
                    })
            })
            .collect()
    }

    #[test]
    fn selects_the_required_packing_at_every_documented_palette_boundary() {
        let cases = [
            (1, 1, 8),
            (2, 1, 8),
            (3, 2, 4),
            (4, 2, 4),
            (5, 4, 2),
            (15, 4, 2),
            (16, 4, 2),
            (17, 8, 1),
            (255, 8, 1),
            (256, 8, 1),
        ];
        for (size, bits, bundle) in cases {
            let packing = IndexPacking::for_palette_size(size).unwrap();
            assert_eq!(packing.bits_per_index(), bits, "palette size {size}");
            assert_eq!(packing.indices_per_pixel(), bundle, "palette size {size}");
        }
    }

    #[test]
    fn documented_palette_sizes_unpack_lsb_first_and_accept_the_last_index() {
        for size in [1, 2, 3, 4, 5, 15, 16, 17, 255, 256] {
            let palette = palette(size);
            let packing = palette.packing();
            let indices = [0, (size - 1) as u8];
            let green = pack_row(&indices, packing);
            let indexed = indexed_image(green.len() as u32, 1, green);
            let image = inverse_color_indexing(&palette, indices.len() as u32, &indexed).unwrap();
            assert_eq!(
                image.pixels(),
                [palette.get(0).unwrap(), palette.get(size - 1).unwrap()]
            );
        }
    }

    #[test]
    fn unpacking_preserves_row_boundaries_and_ignores_unused_tail_bits() {
        let palette = palette(3);
        let packing = palette.packing();
        assert_eq!(packing.indices_per_pixel(), 4);
        // Width five needs two source pixels per row. The high four bits in
        // each second source pixel are deliberately invalid tail data and must
        // not leak into the following row.
        let first_row = [0, 1, 2, 1, 2];
        let second_row = [2, 1, 0, 2, 1];
        let mut green = pack_row(&first_row, packing);
        green[1] |= 0b1111_0000;
        let mut second = pack_row(&second_row, packing);
        second[1] |= 0b1010_0000;
        green.extend(second);
        let indexed = indexed_image(2, 2, green);
        let image = inverse_color_indexing(&palette, 5, &indexed).unwrap();
        let expected: Vec<Rgba> = first_row
            .into_iter()
            .chain(second_row)
            .map(|index| palette.get(usize::from(index)).unwrap())
            .collect();
        assert_eq!(image.pixels(), expected);
    }

    #[test]
    fn unpacking_uses_green_only_and_invalid_indices_are_transparent_black() {
        let palette = palette(3);
        // Two-bit values are 0, 3, 1, 2. Index three is invalid. Distinct red
        // and blue values prove that neither is consulted for palette indices.
        let indexed =
            RgbaImage::new(1, 1, vec![Rgba::new(0b11_10_01_00, 0b10_01_11_00, 0, 99)]).unwrap();
        let image = inverse_color_indexing(&palette, 4, &indexed).unwrap();
        assert_eq!(
            image.pixels(),
            [
                palette.get(0).unwrap(),
                TRANSPARENT_BLACK,
                palette.get(1).unwrap(),
                palette.get(2).unwrap(),
            ]
        );
    }

    #[test]
    fn one_by_n_and_n_by_one_images_cover_all_bundle_modes() {
        for size in [2, 4, 16, 17] {
            let palette = palette(size);
            let packing = palette.packing();
            let vertical = indexed_image(1, 3, vec![0, (size - 1) as u8, 0]);
            let vertical_output = inverse_color_indexing(&palette, 1, &vertical).unwrap();
            assert_eq!(vertical_output.pixels()[1], palette.get(size - 1).unwrap());

            let width = usize::from(packing.indices_per_pixel()) + 1;
            let indices: Vec<u8> = (0..width).map(|index| (index % size) as u8).collect();
            let horizontal = indexed_image(
                packing.packed_width(width as u32).unwrap(),
                1,
                pack_row(&indices, packing),
            );
            let horizontal_output =
                inverse_color_indexing(&palette, width as u32, &horizontal).unwrap();
            let expected: Vec<Rgba> = indices
                .into_iter()
                .map(|index| palette.get(usize::from(index)).unwrap())
                .collect();
            assert_eq!(horizontal_output.pixels(), expected);
        }
    }

    #[test]
    fn palette_deltas_wrap_all_channels_and_preserve_transparent_rgb() {
        let palette = Palette::from_deltas(vec![
            Rgba::new(250, 255, 1, 254),
            Rgba::new(10, 1, 255, 2),
            Rgba::new(1, 0, 0, 0),
        ])
        .unwrap();
        assert_eq!(
            palette.entries(),
            [
                Rgba::new(250, 255, 1, 254),
                Rgba::new(4, 0, 0, 0),
                Rgba::new(5, 0, 0, 0),
            ]
        );
    }

    #[test]
    fn palette_accessors_and_pixel_count_report_validated_state() {
        let palette = palette(3);
        assert!(!palette.is_empty());
        assert_eq!(palette.get(0), Some(Rgba::new(0, 1, 2, 255)));
        assert_eq!(palette.get(2), Some(Rgba::new(2, 3, 4, 253)));
        assert_eq!(palette.get(3), None);
        assert_eq!(pixel_count(0, 9), Ok(0));
        assert_eq!(pixel_count(3, 7), Ok(21));
    }

    #[test]
    fn rejects_bad_palette_sizes_and_wrong_indexed_dimensions() {
        for size in [0, 257] {
            assert!(matches!(
                Palette::new(vec![Rgba::default(); size]),
                Err(ColorIndexError::InvalidPaletteSize { .. })
            ));
        }
        let palette = palette(2);
        let indexed = indexed_image(1, 1, vec![0]);
        assert!(matches!(
            inverse_color_indexing(&palette, 9, &indexed),
            Err(ColorIndexError::IndexedDimensions {
                expected_width: 2,
                ..
            })
        ));
        assert_eq!(
            ColorIndexError::InvalidPaletteSize { actual: 0 }.to_string(),
            "VP8L color-indexing palette has 0 entries; expected 1..=256"
        );
    }
}
