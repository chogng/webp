use crate::allocation::check_transient_indexing_palette_allocation;
use crate::allocation::check_transient_transform_allocation;
use crate::allocation::checked_transform_bytes;
use crate::image_data::decode_image_data;
use crate::pixel::argb_to_rgba;
use webp_core::BitReader;
use webp_core::DecodeError;
use webp_core::DecodeErrorKind;
use webp_core::DecodeLimits;
use webp_core::WorkBudget;
use webp_vp8l::BlockTransformDescriptor;
use webp_vp8l::TransformDescriptor;
use webp_vp8l::TransformListParser;
use webp_vp8l::Vp8lHeader;
use webp_vp8l_color_transform::ColorTransformMultipliers;
use webp_vp8l_indexing::Palette;
use webp_vp8l_transform::PredictorMode;
use webp_vp8l_transform::Rgba;

pub(super) enum DecodedTransform {
    Predictor {
        descriptor: BlockTransformDescriptor,
        mode_pixels: Vec<u32>,
    },
    Color {
        descriptor: BlockTransformDescriptor,
        multipliers: Vec<ColorTransformMultipliers>,
    },
    ColorIndexing {
        descriptor: webp_vp8l::ColorIndexingDescriptor,
        palette: Palette,
    },
    SubtractGreen,
}

pub(super) struct DecodedTransformList {
    pub(super) transforms: Vec<DecodedTransform>,
    pub(super) coded_width: u32,
    pub(super) coded_height: u32,
}

/// Reads the main-level transform list and decodes supported transform
/// subimages immediately.
///
/// Predictor and color descriptors are followed by an `is_level0 = false`
/// entropy image. The nested image has no transform-list flag or meta-Huffman
/// flag; consuming either would desynchronize the main transform list.
pub(super) fn read_transform_list(
    bits: &mut BitReader<'_>,
    budget: &mut WorkBudget,
    header: &Vp8lHeader,
    limits: &DecodeLimits,
    retained_bytes: &mut usize,
) -> Result<DecodedTransformList, DecodeError> {
    let mut parser = TransformListParser::new(header.width, header.height, limits)?;
    let mut transforms = Vec::new();

    loop {
        // Count every transform-list entry, including its terminating bit, as
        // bounded parser work. The empty-list case therefore retains the
        // original one-unit stream-flag accounting.
        budget.consume(1)?;
        match parser.read_next(bits, limits)? {
            None => {
                let (coded_width, coded_height) = parser.image_dimensions();
                return Ok(DecodedTransformList {
                    transforms,
                    coded_width,
                    coded_height,
                });
            }
            Some(TransformDescriptor::SubtractGreen) => {
                transforms.push(DecodedTransform::SubtractGreen)
            }
            Some(TransformDescriptor::Predictor(descriptor)) => {
                let mode_pixels = decode_image_data(
                    bits,
                    descriptor.transform_width,
                    descriptor.transform_height,
                    false,
                    budget,
                    limits,
                    *retained_bytes,
                    0,
                )?;
                validate_predictor_modes(&mode_pixels)?;
                let mode_bytes =
                    mode_pixels
                        .len()
                        .checked_mul(size_of::<u32>())
                        .ok_or_else(|| {
                            DecodeError::new(
                                DecodeErrorKind::LimitExceeded,
                                None,
                                "VP8L predictor mode buffer byte size overflow",
                            )
                        })?;
                *retained_bytes = retained_bytes.checked_add(mode_bytes).ok_or_else(|| {
                    DecodeError::new(
                        DecodeErrorKind::LimitExceeded,
                        None,
                        "VP8L retained transform byte size overflow",
                    )
                })?;
                transforms.push(DecodedTransform::Predictor {
                    descriptor,
                    mode_pixels,
                });
            }
            Some(TransformDescriptor::Color(descriptor)) => {
                let multipliers =
                    decode_color_multipliers(bits, budget, descriptor, limits, *retained_bytes)?;
                let multiplier_bytes = checked_transform_bytes(
                    multipliers.len(),
                    size_of::<ColorTransformMultipliers>(),
                    "VP8L color-transform table byte size overflow",
                )?;
                *retained_bytes =
                    retained_bytes
                        .checked_add(multiplier_bytes)
                        .ok_or_else(|| {
                            DecodeError::new(
                                DecodeErrorKind::LimitExceeded,
                                None,
                                "VP8L retained transform byte size overflow",
                            )
                        })?;
                transforms.push(DecodedTransform::Color {
                    descriptor,
                    multipliers,
                });
            }
            Some(TransformDescriptor::ColorIndexing(descriptor)) => {
                let palette =
                    decode_color_palette(bits, budget, descriptor, limits, *retained_bytes)?;
                let palette_bytes = checked_transform_bytes(
                    palette.len(),
                    size_of::<Rgba>(),
                    "VP8L color-indexing palette byte size overflow",
                )?;
                *retained_bytes = retained_bytes.checked_add(palette_bytes).ok_or_else(|| {
                    DecodeError::new(
                        DecodeErrorKind::LimitExceeded,
                        None,
                        "VP8L retained transform byte size overflow",
                    )
                })?;
                transforms.push(DecodedTransform::ColorIndexing {
                    descriptor,
                    palette,
                });
            }
        }
    }
}

/// Decodes VP8L's one-row, delta-coded color table immediately following a
/// color-indexing descriptor. Keeping the table as [`Palette`] preserves its
/// specified wrapping delta arithmetic and transparent-black handling for
/// out-of-range packed indices.
fn decode_color_palette(
    bits: &mut BitReader<'_>,
    budget: &mut WorkBudget,
    descriptor: webp_vp8l::ColorIndexingDescriptor,
    limits: &DecodeLimits,
    retained_bytes: usize,
) -> Result<Palette, DecodeError> {
    let palette_pixels = decode_image_data(
        bits,
        descriptor.color_table_width(),
        descriptor.color_table_height(),
        false,
        budget,
        limits,
        retained_bytes,
        0,
    )?;
    let packed_bytes = checked_transform_bytes(
        palette_pixels.len(),
        size_of::<u32>(),
        "VP8L color-indexing packed palette byte size overflow",
    )?;
    let palette_bytes = checked_transform_bytes(
        palette_pixels.len(),
        size_of::<Rgba>(),
        "VP8L color-indexing palette byte size overflow",
    )?;
    check_transient_indexing_palette_allocation(
        retained_bytes,
        packed_bytes,
        palette_bytes,
        limits.max_alloc_bytes,
    )?;
    budget.consume(u64::try_from(palette_pixels.len()).map_err(|_| {
        DecodeError::new(
            DecodeErrorKind::LimitExceeded,
            None,
            "VP8L color-indexing palette length exceeds work counter",
        )
    })?)?;

    let mut entries = Vec::new();
    entries
        .try_reserve_exact(palette_pixels.len())
        .map_err(|_| {
            DecodeError::new(
                DecodeErrorKind::AllocationFailed,
                None,
                "VP8L color-indexing palette allocation failed",
            )
        })?;
    for pixel in palette_pixels {
        entries.push(argb_to_rgba(pixel));
    }
    Palette::from_deltas(entries).map_err(|_| {
        DecodeError::new(
            DecodeErrorKind::InvalidBitstream,
            None,
            "VP8L color-indexing palette is invalid",
        )
    })
}

/// Decodes and converts a VP8L color-transform subimage to its three-byte
/// coefficient table.  A transform pixel is packed as `0xAARRGGBB`; VP8L
/// assigns B to green-to-red, G to green-to-blue, and R to red-to-blue. Alpha
/// is intentionally ignored.
fn decode_color_multipliers(
    bits: &mut BitReader<'_>,
    budget: &mut WorkBudget,
    descriptor: BlockTransformDescriptor,
    limits: &DecodeLimits,
    retained_bytes: usize,
) -> Result<Vec<ColorTransformMultipliers>, DecodeError> {
    let color_pixels = decode_image_data(
        bits,
        descriptor.transform_width,
        descriptor.transform_height,
        false,
        budget,
        limits,
        retained_bytes,
        0,
    )?;
    let packed_bytes = checked_transform_bytes(
        color_pixels.len(),
        size_of::<u32>(),
        "VP8L color-transform packed table byte size overflow",
    )?;
    let multiplier_bytes = checked_transform_bytes(
        color_pixels.len(),
        size_of::<ColorTransformMultipliers>(),
        "VP8L color-transform multiplier table byte size overflow",
    )?;
    check_transient_transform_allocation(
        retained_bytes,
        packed_bytes,
        multiplier_bytes,
        limits.max_alloc_bytes,
    )?;
    budget.consume(u64::try_from(color_pixels.len()).map_err(|_| {
        DecodeError::new(
            DecodeErrorKind::LimitExceeded,
            None,
            "VP8L color-transform table length exceeds work counter",
        )
    })?)?;

    let mut multipliers = Vec::new();
    multipliers
        .try_reserve_exact(color_pixels.len())
        .map_err(|_| {
            DecodeError::new(
                DecodeErrorKind::AllocationFailed,
                None,
                "VP8L color-transform multiplier allocation failed",
            )
        })?;
    for pixel in color_pixels {
        multipliers.push(ColorTransformMultipliers::new(
            pixel as u8 as i8,
            (pixel >> 8) as u8 as i8,
            (pixel >> 16) as u8 as i8,
        ));
    }
    Ok(multipliers)
}

fn validate_predictor_modes(pixels: &[u32]) -> Result<(), DecodeError> {
    for &pixel in pixels {
        PredictorMode::try_from(((pixel >> 8) & 0x0f) as u8).map_err(|_| {
            DecodeError::new(
                DecodeErrorKind::InvalidBitstream,
                None,
                "VP8L predictor mode must be in 0..=13",
            )
        })?;
    }
    Ok(())
}
