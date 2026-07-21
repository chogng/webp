#![forbid(unsafe_code)]
//! A bounded decoder for static VP8L images.
//!
//! The decoder supports VP8L's four transforms, color cache, literal and
//! backward-reference entropy symbols, and spatial meta-Huffman groups. The
//! output uses straight RGBA byte order.

use webp_core::{
    BitReader, DecodeError, DecodeErrorKind, DecodeLimits, WorkBudget, checked_image_bytes,
};
use webp_vp8l::{
    BlockTransformDescriptor, HEADER_LEN, TransformDescriptor, TransformListParser, Vp8lHeader,
    parse_header,
};
use webp_vp8l_color_cache::{ColorCache, MAX_COLOR_CACHE_BITS, MIN_COLOR_CACHE_BITS};
use webp_vp8l_color_transform::ColorTransformMultipliers;
use webp_vp8l_entropy::{
    copy_lz77_pixels_preallocated, decode_distance_shifted, decode_length_shifted,
    distance_code_to_distance,
};
use webp_vp8l_huffman::{
    FastHuffmanTable, MAX_SECONDARY_TABLE_STORAGE_BYTES, ROOT_TABLE_STORAGE_BYTES,
    read_huffman_code,
};
use webp_vp8l_indexing::{Palette, TRANSPARENT_BLACK};
use webp_vp8l_transform::{PredictorMode, Rgba};

const GREEN_ALPHABET_SIZE: usize = 256 + 24;
const CHANNEL_ALPHABET_SIZE: usize = 256;
const DISTANCE_ALPHABET_SIZE: usize = 40;

/// A decoded straight/unpremultiplied RGBA image.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiteralImage {
    /// Fixed VP8L image information.
    pub header: Vp8lHeader,
    /// Pixels in row-major RGBA8 byte order.
    pub rgba: Vec<u8>,
}

#[cfg(test)]
#[derive(Default)]
struct DecodePhaseTimings {
    entropy: std::time::Duration,
    rgba_conversion: std::time::Duration,
    predictor: std::time::Duration,
    entropy_paths: EntropyPathCounters,
}

#[cfg(test)]
#[derive(Clone, Copy, Default)]
struct EntropyPathCounters {
    literal_pixels: u64,
    batched_literals: u64,
    cache_hits: u64,
    copy_commands: u64,
    copy_pixels: u64,
    meta_runs: u64,
}

#[cfg(test)]
impl EntropyPathCounters {
    fn add_assign(&mut self, other: Self) {
        self.literal_pixels += other.literal_pixels;
        self.batched_literals += other.batched_literals;
        self.cache_hits += other.cache_hits;
        self.copy_commands += other.copy_commands;
        self.copy_pixels += other.copy_pixels;
        self.meta_runs += other.meta_runs;
    }
}

#[cfg(test)]
std::thread_local! {
    static ENTROPY_PATH_COUNTERS: std::cell::Cell<EntropyPathCounters> =
        std::cell::Cell::new(EntropyPathCounters {
            literal_pixels: 0,
            batched_literals: 0,
            cache_hits: 0,
            copy_commands: 0,
            copy_pixels: 0,
            meta_runs: 0,
        });
}

#[cfg(test)]
fn reset_entropy_path_counters() {
    ENTROPY_PATH_COUNTERS.with(|counters| counters.set(EntropyPathCounters::default()));
}

#[cfg(test)]
fn entropy_path_counters() -> EntropyPathCounters {
    ENTROPY_PATH_COUNTERS.with(std::cell::Cell::get)
}

#[cfg(test)]
fn record_entropy_path(update: impl FnOnce(&mut EntropyPathCounters)) {
    ENTROPY_PATH_COUNTERS.with(|counters| {
        let mut current = counters.get();
        update(&mut current);
        counters.set(current);
    });
}

#[cfg(test)]
#[path = "predictor_benchmark_tests.rs"]
mod predictor_benchmark_tests;

/// Decodes a standalone static VP8L stream to straight RGBA8.
///
/// The input begins with the five-byte VP8L fixed header.
pub fn decode_vp8l(data: &[u8], limits: &DecodeLimits) -> Result<LiteralImage, DecodeError> {
    decode_no_transform(data, limits)
}

/// Backwards-compatible name for [`decode_vp8l`].
pub fn decode_literal_only(
    data: &[u8],
    limits: &DecodeLimits,
) -> Result<LiteralImage, DecodeError> {
    decode_vp8l(data, limits)
}

/// Decodes a standalone static VP8L stream.
///
/// Literal pixels, green-alphabet backward-reference symbols, and color-cache
/// references are supported. The transform list may be empty or contain
/// subtract-green, predictor, color, and color-indexing transforms. Main
/// images may use spatial meta-Huffman groups; transform subimages cannot.
/// Internally decoded samples are packed as `0xAARRGGBB` until entropy
/// expansion is complete, then inverse-transformed and emitted in RGBA byte
/// order.
pub fn decode_no_transform(
    data: &[u8],
    limits: &DecodeLimits,
) -> Result<LiteralImage, DecodeError> {
    decode_no_transform_inner(
        data,
        limits,
        #[cfg(test)]
        None,
    )
}

#[cfg(test)]
fn decode_no_transform_profiled(
    data: &[u8],
    limits: &DecodeLimits,
    timings: &mut DecodePhaseTimings,
) -> Result<LiteralImage, DecodeError> {
    decode_no_transform_inner(data, limits, Some(timings))
}

fn decode_no_transform_inner(
    data: &[u8],
    limits: &DecodeLimits,
    #[cfg(test)] mut timings: Option<&mut DecodePhaseTimings>,
) -> Result<LiteralImage, DecodeError> {
    let header = parse_header(data, limits)?;
    let rgba_len = checked_image_bytes(header.width, header.height, 4)?;
    if rgba_len > limits.max_alloc_bytes {
        return Err(DecodeError::new(
            DecodeErrorKind::LimitExceeded,
            None,
            "RGBA output exceeds configured allocation limit",
        ));
    }

    let mut bits = BitReader::with_bit_position(data, HEADER_LEN * 8)?;
    let mut budget = limits.work_budget();

    let mut retained_transform_bytes = 0_usize;
    let decoded_transforms = read_supported_transforms(
        &mut bits,
        &mut budget,
        &header,
        limits,
        &mut retained_transform_bytes,
    )?;
    #[cfg(test)]
    let entropy_started = std::time::Instant::now();
    #[cfg(test)]
    if timings.is_some() {
        reset_entropy_path_counters();
    }
    let output = decode_entropy_image(
        &mut bits,
        decoded_transforms.coded_width,
        decoded_transforms.coded_height,
        true,
        &mut budget,
        limits,
        retained_transform_bytes,
        rgba_len,
    )?;
    #[cfg(test)]
    if let Some(timings) = timings.as_mut() {
        timings.entropy += entropy_started.elapsed();
        timings.entropy_paths.add_assign(entropy_path_counters());
    }
    let mut output = TransformPixels::Argb(output);

    for transform in decoded_transforms.transforms.iter().rev() {
        match transform {
            DecodedTransform::SubtractGreen => inverse_subtract_green_argb(output.argb_mut()?),
            DecodedTransform::Predictor {
                descriptor,
                mode_pixels,
            } => {
                #[cfg(test)]
                let predictor_started = std::time::Instant::now();
                output.inverse_predictor(*descriptor, mode_pixels)?;
                #[cfg(test)]
                if let Some(timings) = timings.as_mut() {
                    timings.predictor += predictor_started.elapsed();
                }
            }
            DecodedTransform::Color {
                descriptor,
                multipliers,
            } => inverse_color_argb(output.argb_mut()?, *descriptor, multipliers)?,
            DecodedTransform::ColorIndexing {
                descriptor,
                palette,
            } => inverse_color_indexing_argb(
                output.argb_mut()?,
                *descriptor,
                palette,
                retained_transform_bytes,
                rgba_len,
                limits.max_alloc_bytes,
            )?,
        }
    }
    drop(decoded_transforms);
    #[cfg(test)]
    let conversion_started = std::time::Instant::now();
    let rgba = output.into_rgba(rgba_len)?;
    #[cfg(test)]
    if let Some(timings) = timings.as_mut() {
        timings.rgba_conversion += conversion_started.elapsed();
    }

    Ok(LiteralImage { header, rgba })
}

/// Decodes VP8L's entropy image syntax at either nesting level.
///
/// A main-level image may carry a spatial meta-Huffman image. Predictor and
/// transform subimages are `is_level0 = false`, so their Huffman stream begins
/// directly after the color-cache declaration and cannot recursively carry
/// meta-Huffman data.
#[allow(clippy::too_many_arguments)]
fn decode_entropy_image(
    bits: &mut BitReader<'_>,
    width: u32,
    height: u32,
    is_level0: bool,
    budget: &mut WorkBudget,
    limits: &DecodeLimits,
    retained_bytes: usize,
    final_rgba_bytes: usize,
) -> Result<Vec<u32>, DecodeError> {
    budget.consume(1)?;
    let color_cache_bits = if bits.read_bit()? {
        budget.consume(1)?;
        Some(bits.read_bits(4)? as u8)
    } else {
        None
    };

    let color_cache_size = color_cache_size(color_cache_bits)?;
    let pixels = pixel_count(width, height)?;
    check_allocation_budget(
        pixels,
        final_rgba_bytes,
        color_cache_size,
        retained_bytes,
        limits.max_alloc_bytes,
    )?;

    let codes = if is_level0 {
        budget.consume(1)?;
        if bits.read_bit()? {
            EntropyCodes::Meta(read_meta_huffman_codes(
                bits,
                width,
                height,
                color_cache_size,
                budget,
                limits,
                retained_bytes,
                final_rgba_bytes,
            )?)
        } else {
            EntropyCodes::Single(box_huffman_codes(read_huffman_codes(
                bits,
                budget,
                color_cache_size,
            )?)?)
        }
    } else {
        EntropyCodes::Single(box_huffman_codes(read_huffman_codes(
            bits,
            budget,
            color_cache_size,
        )?)?)
    };
    let mut code_cursor = codes.cursor(width)?;
    let mut output = PixelOutput::new(color_cache_bits, pixels)?;
    let mut shifted_bits = bits.shifted();

    while output.len() < pixels {
        let (codes, run_end) = code_cursor.run_for_pixel(output.len(), pixels)?;
        #[cfg(test)]
        if code_cursor.is_meta() {
            record_entropy_path(|paths| paths.meta_runs += 1);
        }
        decode_entropy_run(
            codes,
            &mut shifted_bits,
            &mut output,
            run_end,
            pixels,
            width,
            color_cache_size,
            budget,
        )?;
    }

    drop(shifted_bits);
    Ok(output.into_pixels())
}

#[allow(clippy::too_many_arguments)]
fn decode_entropy_run(
    codes: &HuffmanCodes,
    shifted_bits: &mut webp_core::ShiftedBitReader<'_, '_>,
    output: &mut PixelOutput,
    run_end: usize,
    pixels: usize,
    width: u32,
    color_cache_size: usize,
    budget: &mut WorkBudget,
) -> Result<(), DecodeError> {
    while output.len() < run_end {
        shifted_bits.fill();
        let green = match decode_green_or_literal(codes, shifted_bits, budget)? {
            GreenOrLiteral::Literal(color) => {
                #[cfg(test)]
                record_entropy_path(|paths| {
                    paths.literal_pixels += 1;
                    paths.batched_literals += 1;
                });
                output.emit_literal(color)?;
                continue;
            }
            GreenOrLiteral::Green(green) => green,
        };
        if green < CHANNEL_ALPHABET_SIZE {
            // Green has already consumed one symbol work unit. Charge the
            // three literal channels together so the hot path performs one
            // checked budget decrement instead of three more.
            budget.consume(3)?;
            let red = usize::from(codes.red.decode(shifted_bits)?);
            let blue = usize::from(codes.blue.decode(shifted_bits)?);
            if shifted_bits.available_bits() < 15 {
                shifted_bits.fill();
            }
            let alpha = usize::from(codes.alpha.decode(shifted_bits)?);
            debug_assert!(red < CHANNEL_ALPHABET_SIZE);
            debug_assert!(blue < CHANNEL_ALPHABET_SIZE);
            debug_assert!(alpha < CHANNEL_ALPHABET_SIZE);
            #[cfg(test)]
            record_entropy_path(|paths| paths.literal_pixels += 1);
            output.emit_literal(pack_argb(red as u8, green as u8, blue as u8, alpha as u8))?;
            continue;
        }

        if green >= GREEN_ALPHABET_SIZE {
            let cache_index = green - GREEN_ALPHABET_SIZE;
            if cache_index >= color_cache_size {
                return Err(DecodeError::new(
                    DecodeErrorKind::InvalidBitstream,
                    None,
                    "VP8L color-cache symbol exceeds cache size",
                ));
            }
            #[cfg(test)]
            record_entropy_path(|paths| paths.cache_hits += 1);
            output.emit_cache_hit(cache_index)?;
            continue;
        }

        let length_prefix = u8::try_from(green - CHANNEL_ALPHABET_SIZE).map_err(|_| {
            DecodeError::new(
                DecodeErrorKind::InvalidBitstream,
                None,
                "VP8L length prefix does not fit u8",
            )
        })?;
        let length = decode_length_shifted(shifted_bits, budget, length_prefix)?;
        let distance_prefix = decode_fast_symbol(&codes.distance, shifted_bits, budget)?;
        let distance_prefix = u8::try_from(distance_prefix).map_err(|_| {
            DecodeError::new(
                DecodeErrorKind::InvalidBitstream,
                None,
                "VP8L distance prefix does not fit u8",
            )
        })?;
        let distance_code = decode_distance_shifted(shifted_bits, budget, distance_prefix)?;
        let distance = distance_code_to_distance(distance_code, width)?;
        #[cfg(test)]
        record_entropy_path(|paths| {
            paths.copy_commands += 1;
            paths.copy_pixels += length as u64;
        });
        output.copy_lz77(length, distance, pixels, budget)?;
    }
    Ok(())
}

enum DecodedTransform {
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

struct DecodedTransforms {
    transforms: Vec<DecodedTransform>,
    coded_width: u32,
    coded_height: u32,
}

/// Reads the main-level transform list and decodes supported transform
/// subimages immediately.
///
/// Predictor and color descriptors are followed by an `is_level0 = false`
/// entropy image. The nested image has no transform-list flag or meta-Huffman
/// flag; consuming either would desynchronize the main transform list.
fn read_supported_transforms(
    bits: &mut BitReader<'_>,
    budget: &mut WorkBudget,
    header: &Vp8lHeader,
    limits: &DecodeLimits,
    retained_bytes: &mut usize,
) -> Result<DecodedTransforms, DecodeError> {
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
                return Ok(DecodedTransforms {
                    transforms,
                    coded_width,
                    coded_height,
                });
            }
            Some(TransformDescriptor::SubtractGreen) => {
                transforms.push(DecodedTransform::SubtractGreen)
            }
            Some(TransformDescriptor::Predictor(descriptor)) => {
                let mode_pixels = decode_entropy_image(
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
    let palette_pixels = decode_entropy_image(
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
    let color_pixels = decode_entropy_image(
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

/// Bounds the allocations that coexist while entropy output becomes RGBA.
///
/// The packed ARGB output, optional color-cache entries, and final RGBA bytes
/// all coexist while the decoder allocates the final image. This deliberately
/// treats vector capacities as their maximum configured sizes, avoiding an
/// allocation-limit bypass through a tiny image paired with a large cache.
fn check_allocation_budget(
    pixels: usize,
    rgba_len: usize,
    color_cache_size: usize,
    retained_bytes: usize,
    max_alloc_bytes: usize,
) -> Result<(), DecodeError> {
    let packed_bytes = pixels.checked_mul(size_of::<u32>()).ok_or_else(|| {
        DecodeError::new(
            DecodeErrorKind::LimitExceeded,
            None,
            "packed VP8L output byte size overflow",
        )
    })?;
    let cache_bytes = color_cache_size
        .checked_mul(size_of::<u32>())
        .ok_or_else(|| {
            DecodeError::new(
                DecodeErrorKind::LimitExceeded,
                None,
                "VP8L color-cache byte size overflow",
            )
        })?;
    let total = retained_bytes
        .checked_add(packed_bytes)
        .and_then(|value| value.checked_add(cache_bytes))
        .and_then(|value| value.checked_add(rgba_len))
        .ok_or_else(|| {
            DecodeError::new(
                DecodeErrorKind::LimitExceeded,
                None,
                "VP8L decoder allocation budget overflow",
            )
        })?;
    if total > max_alloc_bytes {
        return Err(DecodeError::new(
            DecodeErrorKind::LimitExceeded,
            None,
            "VP8L decoder allocations exceed configured allocation limit",
        ));
    }
    Ok(())
}

fn checked_transform_bytes(
    entries: usize,
    entry_size: usize,
    overflow_message: &'static str,
) -> Result<usize, DecodeError> {
    entries
        .checked_mul(entry_size)
        .ok_or_else(|| DecodeError::new(DecodeErrorKind::LimitExceeded, None, overflow_message))
}

/// Verifies the brief conversion overlap between an entropy-decoded packed
/// color subimage and its compact coefficient table.
fn check_transient_transform_allocation(
    retained_bytes: usize,
    packed_bytes: usize,
    multiplier_bytes: usize,
    max_alloc_bytes: usize,
) -> Result<(), DecodeError> {
    let total = retained_bytes
        .checked_add(packed_bytes)
        .and_then(|value| value.checked_add(multiplier_bytes))
        .ok_or_else(|| {
            DecodeError::new(
                DecodeErrorKind::LimitExceeded,
                None,
                "VP8L color-transform conversion allocation size overflow",
            )
        })?;
    if total > max_alloc_bytes {
        return Err(DecodeError::new(
            DecodeErrorKind::LimitExceeded,
            None,
            "VP8L color-transform conversion exceeds allocation limit",
        ));
    }
    Ok(())
}

/// Bounds the brief overlap while a decoded packed palette becomes the
/// retained [`Palette`] representation.
fn check_transient_indexing_palette_allocation(
    retained_bytes: usize,
    packed_bytes: usize,
    palette_bytes: usize,
    max_alloc_bytes: usize,
) -> Result<(), DecodeError> {
    let total = retained_bytes
        .checked_add(packed_bytes)
        .and_then(|value| value.checked_add(palette_bytes))
        .ok_or_else(|| {
            DecodeError::new(
                DecodeErrorKind::LimitExceeded,
                None,
                "VP8L color-indexing palette conversion allocation size overflow",
            )
        })?;
    if total > max_alloc_bytes {
        return Err(DecodeError::new(
            DecodeErrorKind::LimitExceeded,
            None,
            "VP8L color-indexing palette conversion exceeds allocation limit",
        ));
    }
    Ok(())
}

fn pixel_count(width: u32, height: u32) -> Result<usize, DecodeError> {
    usize::try_from(u64::from(width) * u64::from(height)).map_err(|_| {
        DecodeError::new(
            DecodeErrorKind::LimitExceeded,
            None,
            "image pixel count does not fit platform usize",
        )
    })
}

fn color_cache_size(color_cache_bits: Option<u8>) -> Result<usize, DecodeError> {
    match color_cache_bits {
        None => Ok(0),
        Some(cache_bits) => {
            if !(MIN_COLOR_CACHE_BITS..=MAX_COLOR_CACHE_BITS).contains(&cache_bits) {
                return Err(DecodeError::new(
                    DecodeErrorKind::InvalidBitstream,
                    None,
                    "VP8L color-cache bits must be in 1..=11",
                ));
            }
            Ok(1_usize << cache_bits)
        }
    }
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

const fn pack_argb(red: u8, green: u8, blue: u8, alpha: u8) -> u32 {
    ((alpha as u32) << 24) | ((red as u32) << 16) | ((green as u32) << 8) | (blue as u32)
}

const fn unpack_rgba(pixel: u32) -> [u8; 4] {
    [
        (pixel >> 16) as u8,
        (pixel >> 8) as u8,
        pixel as u8,
        (pixel >> 24) as u8,
    ]
}

/// Internal transform storage with explicit intermediate layout states.
///
/// Entropy and color indexing naturally operate on VP8L's packed ARGB words,
/// while predictor reconstruction benefits from channel-contiguous RGBA byte
/// lanes. Keeping the conversion at this private boundary leaves the public
/// transform crate and its other callers unchanged.
enum TransformPixels {
    Argb(Vec<u32>),
    Rgba(Vec<u8>),
}

impl TransformPixels {
    fn argb_mut(&mut self) -> Result<&mut Vec<u32>, DecodeError> {
        if let Self::Rgba(bytes) = self {
            if !bytes.len().is_multiple_of(4) {
                return Err(DecodeError::new(
                    DecodeErrorKind::InvalidBitstream,
                    None,
                    "VP8L RGBA transform buffer has unexpected length",
                ));
            }
            let mut packed = Vec::new();
            packed.try_reserve_exact(bytes.len() / 4).map_err(|_| {
                DecodeError::new(
                    DecodeErrorKind::AllocationFailed,
                    None,
                    "VP8L packed transform allocation failed",
                )
            })?;
            for pixel in bytes.chunks_exact(4) {
                packed.push(pack_argb(pixel[0], pixel[1], pixel[2], pixel[3]));
            }
            *self = Self::Argb(packed);
        }
        match self {
            Self::Argb(pixels) => Ok(pixels),
            Self::Rgba(_) => unreachable!("RGBA transform buffer was converted to ARGB"),
        }
    }

    fn rgba_mut(&mut self) -> Result<&mut Vec<u8>, DecodeError> {
        if let Self::Argb(pixels) = self {
            let actual_len = pixels.len().checked_mul(4).ok_or_else(|| {
                DecodeError::new(
                    DecodeErrorKind::LimitExceeded,
                    None,
                    "VP8L RGBA transform length overflow",
                )
            })?;
            let mut bytes = Vec::new();
            bytes.try_reserve_exact(actual_len).map_err(|_| {
                DecodeError::new(
                    DecodeErrorKind::AllocationFailed,
                    None,
                    "RGBA output allocation failed",
                )
            })?;
            for &pixel in pixels.iter() {
                bytes.extend_from_slice(&unpack_rgba(pixel));
            }
            *self = Self::Rgba(bytes);
        }
        match self {
            Self::Rgba(bytes) => Ok(bytes),
            Self::Argb(_) => unreachable!("ARGB transform buffer was converted to RGBA"),
        }
    }

    fn inverse_predictor(
        &mut self,
        descriptor: BlockTransformDescriptor,
        mode_pixels: &[u32],
    ) -> Result<(), DecodeError> {
        let converted = match self {
            Self::Argb(pixels) => Some(inverse_predictor_argb_to_rgba(
                pixels,
                descriptor,
                mode_pixels,
            )?),
            Self::Rgba(bytes) => {
                inverse_predictor_rgba(bytes, descriptor, mode_pixels)?;
                None
            }
        };
        if let Some(bytes) = converted {
            *self = Self::Rgba(bytes);
        }
        Ok(())
    }

    fn into_rgba(mut self, expected_rgba_len: usize) -> Result<Vec<u8>, DecodeError> {
        self.rgba_mut()?;
        match self {
            Self::Rgba(bytes) if bytes.len() == expected_rgba_len => Ok(bytes),
            Self::Rgba(_) => Err(DecodeError::new(
                DecodeErrorKind::InvalidBitstream,
                None,
                "VP8L final RGBA buffer has unexpected length",
            )),
            Self::Argb(_) => unreachable!("ARGB transform buffer was converted to RGBA"),
        }
    }
}

/// Reverses VP8L color indexing in packed ARGB form.  The decoder keeps the
/// narrow entropy output alive until the expanded row-major output is fully
/// initialized, so this explicitly accounts for both buffers plus the final
/// RGBA allocation and all retained transform tables.
fn inverse_color_indexing_argb(
    pixels: &mut Vec<u32>,
    descriptor: webp_vp8l::ColorIndexingDescriptor,
    palette: &Palette,
    retained_bytes: usize,
    final_rgba_bytes: usize,
    max_alloc_bytes: usize,
) -> Result<(), DecodeError> {
    let packing = palette.packing();
    let indices_per_pixel = usize::from(packing.indices_per_pixel());
    let expected_bundle = 1_usize << descriptor.width_bits;
    if indices_per_pixel != expected_bundle {
        return Err(DecodeError::new(
            DecodeErrorKind::InvalidBitstream,
            None,
            "VP8L color-indexing descriptor does not match palette packing",
        ));
    }
    if descriptor.image_width_after
        != palette
            .packing()
            .packed_width(descriptor.image_width_before)
            .map_err(|_| {
                DecodeError::new(
                    DecodeErrorKind::InvalidBitstream,
                    None,
                    "VP8L color-indexing packed width is invalid",
                )
            })?
    {
        return Err(DecodeError::new(
            DecodeErrorKind::InvalidBitstream,
            None,
            "VP8L color-indexing packed width does not match descriptor",
        ));
    }

    let packed_pixels = pixel_count(descriptor.image_width_after, descriptor.image_height)?;
    if pixels.len() != packed_pixels {
        return Err(DecodeError::new(
            DecodeErrorKind::InvalidBitstream,
            None,
            "VP8L color-indexing output length does not match packed dimensions",
        ));
    }
    let expanded_pixels = pixel_count(descriptor.image_width_before, descriptor.image_height)?;
    let packed_bytes = checked_transform_bytes(
        packed_pixels,
        size_of::<u32>(),
        "VP8L color-indexing packed image byte size overflow",
    )?;
    let expanded_bytes = checked_transform_bytes(
        expanded_pixels,
        size_of::<u32>(),
        "VP8L color-indexing expanded image byte size overflow",
    )?;
    let total = retained_bytes
        .checked_add(packed_bytes)
        .and_then(|value| value.checked_add(expanded_bytes))
        .and_then(|value| value.checked_add(final_rgba_bytes))
        .ok_or_else(|| {
            DecodeError::new(
                DecodeErrorKind::LimitExceeded,
                None,
                "VP8L color-indexing expansion allocation size overflow",
            )
        })?;
    if total > max_alloc_bytes {
        return Err(DecodeError::new(
            DecodeErrorKind::LimitExceeded,
            None,
            "VP8L color-indexing expansion exceeds allocation limit",
        ));
    }

    let width_before = usize::try_from(descriptor.image_width_before).map_err(|_| {
        DecodeError::new(
            DecodeErrorKind::LimitExceeded,
            None,
            "VP8L color-indexing image width does not fit usize",
        )
    })?;
    let width_after = usize::try_from(descriptor.image_width_after).map_err(|_| {
        DecodeError::new(
            DecodeErrorKind::LimitExceeded,
            None,
            "VP8L color-indexing packed width does not fit usize",
        )
    })?;
    let bits_per_index = u32::from(packing.bits_per_index());
    let mask = (1_u16 << bits_per_index) - 1;

    let mut expanded = Vec::new();
    expanded.try_reserve_exact(expanded_pixels).map_err(|_| {
        DecodeError::new(
            DecodeErrorKind::AllocationFailed,
            None,
            "VP8L color-indexing expanded output allocation failed",
        )
    })?;
    for row in pixels.chunks_exact(width_after) {
        for x in 0..width_before {
            let packed = (row[x / indices_per_pixel] >> 8) as u8;
            let shift = u32::try_from(x % indices_per_pixel)
                .expect("VP8L color-indexing shift fits u32")
                * bits_per_index;
            let index = usize::from((u16::from(packed) >> shift) & mask);
            let color = palette.get(index).unwrap_or(TRANSPARENT_BLACK);
            expanded.push(pack_argb(color.red, color.green, color.blue, color.alpha));
        }
    }
    *pixels = expanded;
    Ok(())
}

/// Inverts subtract-green directly in the packed ARGB representation.
///
/// Keeping this as a packed-pixel helper avoids allocating a second image
/// buffer solely to adapt to the transform crate's RGBA image type. The green
/// and alpha lanes are unchanged, while red and blue add green modulo 256.
fn inverse_subtract_green_argb(pixels: &mut [u32]) {
    for pixel in pixels {
        let green = (*pixel >> 8) as u8;
        let red = ((*pixel >> 16) as u8).wrapping_add(green);
        let blue = (*pixel as u8).wrapping_add(green);
        *pixel = (*pixel & 0xff00_ff00) | (u32::from(red) << 16) | u32::from(blue);
    }
}

/// Inverts a color transform in packed ARGB order without a second image
/// buffer.  The coefficient table has already been validated against the
/// descriptor during transform-subimage decoding.
fn inverse_color_argb(
    pixels: &mut [u32],
    descriptor: BlockTransformDescriptor,
    multipliers: &[ColorTransformMultipliers],
) -> Result<(), DecodeError> {
    let width = usize::try_from(descriptor.image_width).map_err(|_| {
        DecodeError::new(
            DecodeErrorKind::LimitExceeded,
            None,
            "VP8L color-transform image width does not fit usize",
        )
    })?;
    let height = usize::try_from(descriptor.image_height).map_err(|_| {
        DecodeError::new(
            DecodeErrorKind::LimitExceeded,
            None,
            "VP8L color-transform image height does not fit usize",
        )
    })?;
    let expected_pixels = width.checked_mul(height).ok_or_else(|| {
        DecodeError::new(
            DecodeErrorKind::LimitExceeded,
            None,
            "VP8L color-transform image pixel count overflow",
        )
    })?;
    if pixels.len() != expected_pixels {
        return Err(DecodeError::new(
            DecodeErrorKind::InvalidBitstream,
            None,
            "VP8L color-transform output length does not match image dimensions",
        ));
    }

    let table_width = usize::try_from(descriptor.transform_width).map_err(|_| {
        DecodeError::new(
            DecodeErrorKind::LimitExceeded,
            None,
            "VP8L color-transform table width does not fit usize",
        )
    })?;
    let table_height = usize::try_from(descriptor.transform_height).map_err(|_| {
        DecodeError::new(
            DecodeErrorKind::LimitExceeded,
            None,
            "VP8L color-transform table height does not fit usize",
        )
    })?;
    let expected_multipliers = table_width.checked_mul(table_height).ok_or_else(|| {
        DecodeError::new(
            DecodeErrorKind::LimitExceeded,
            None,
            "VP8L color-transform table pixel count overflow",
        )
    })?;
    if multipliers.len() != expected_multipliers {
        return Err(DecodeError::new(
            DecodeErrorKind::InvalidBitstream,
            None,
            "VP8L color-transform table has unexpected dimensions",
        ));
    }

    let block_size = usize::try_from(descriptor.block_size()).map_err(|_| {
        DecodeError::new(
            DecodeErrorKind::LimitExceeded,
            None,
            "VP8L color-transform block size does not fit usize",
        )
    })?;
    for y in 0..height {
        for x in 0..width {
            let pixel_index = y * width + x;
            let table_index = (y / block_size) * table_width + (x / block_size);
            pixels[pixel_index] =
                inverse_color_pixel_argb(pixels[pixel_index], multipliers[table_index]);
        }
    }
    Ok(())
}

/// Applies libwebp's scalar VP8L inverse color arithmetic to one packed pixel.
/// Both green and the reconstructed red fed to the blue multiplier are signed
/// bytes; the red result is reduced modulo 256 before the final multiplication.
const fn inverse_color_pixel_argb(pixel: u32, multipliers: ColorTransformMultipliers) -> u32 {
    let green = ((pixel >> 8) as u8) as i8;
    let mut red = (pixel >> 16) as u8 as i32;
    let mut blue = pixel as u8 as i32;
    red = (red + color_delta(multipliers.green_to_red, green)) & 0xff;
    blue += color_delta(multipliers.green_to_blue, green);
    blue += color_delta(multipliers.red_to_blue, red as u8 as i8);
    blue &= 0xff;
    (pixel & 0xff00_ff00) | ((red as u32) << 16) | (blue as u32)
}

const fn color_delta(multiplier: i8, channel: i8) -> i32 {
    ((multiplier as i32) * (channel as i32)) >> 5
}

/// Validated dimensions shared by both predictor storage paths.
struct PredictorLayout {
    width: usize,
    height: usize,
    row_bytes: usize,
    mode_width: usize,
    block_size: usize,
}

fn predictor_layout(
    descriptor: BlockTransformDescriptor,
    mode_pixels: &[u32],
) -> Result<PredictorLayout, DecodeError> {
    let width = usize::try_from(descriptor.image_width).map_err(|_| {
        DecodeError::new(
            DecodeErrorKind::LimitExceeded,
            None,
            "VP8L predictor image width does not fit usize",
        )
    })?;
    let height = usize::try_from(descriptor.image_height).map_err(|_| {
        DecodeError::new(
            DecodeErrorKind::LimitExceeded,
            None,
            "VP8L predictor image height does not fit usize",
        )
    })?;
    let row_bytes = width.checked_mul(4).ok_or_else(|| {
        DecodeError::new(
            DecodeErrorKind::LimitExceeded,
            None,
            "VP8L predictor row byte size overflow",
        )
    })?;
    let mode_width = usize::try_from(descriptor.transform_width).map_err(|_| {
        DecodeError::new(
            DecodeErrorKind::LimitExceeded,
            None,
            "VP8L predictor mode width does not fit usize",
        )
    })?;
    let expected_modes = mode_width
        .checked_mul(usize::try_from(descriptor.transform_height).map_err(|_| {
            DecodeError::new(
                DecodeErrorKind::LimitExceeded,
                None,
                "VP8L predictor mode height does not fit usize",
            )
        })?)
        .ok_or_else(|| {
            DecodeError::new(
                DecodeErrorKind::LimitExceeded,
                None,
                "VP8L predictor mode pixel count overflow",
            )
        })?;
    if mode_pixels.len() != expected_modes {
        return Err(DecodeError::new(
            DecodeErrorKind::InvalidBitstream,
            None,
            "VP8L predictor mode image has unexpected dimensions",
        ));
    }
    let block_size = usize::try_from(descriptor.block_size()).map_err(|_| {
        DecodeError::new(
            DecodeErrorKind::LimitExceeded,
            None,
            "VP8L predictor block size does not fit usize",
        )
    })?;
    Ok(PredictorLayout {
        width,
        height,
        row_bytes,
        mode_width,
        block_size,
    })
}

/// Converts packed residuals one row at a time and reconstructs each row while
/// it is still cache-hot, avoiding a separate full-frame RGBA conversion pass.
fn inverse_predictor_argb_to_rgba(
    pixels: &[u32],
    descriptor: BlockTransformDescriptor,
    mode_pixels: &[u32],
) -> Result<Vec<u8>, DecodeError> {
    let layout = predictor_layout(descriptor, mode_pixels)?;
    let expected_pixels = layout.width.checked_mul(layout.height).ok_or_else(|| {
        DecodeError::new(
            DecodeErrorKind::LimitExceeded,
            None,
            "VP8L predictor image pixel count overflow",
        )
    })?;
    if pixels.len() != expected_pixels {
        return Err(DecodeError::new(
            DecodeErrorKind::InvalidBitstream,
            None,
            "VP8L predictor output length does not match image dimensions",
        ));
    }
    let expected_bytes = layout
        .row_bytes
        .checked_mul(layout.height)
        .ok_or_else(|| {
            DecodeError::new(
                DecodeErrorKind::LimitExceeded,
                None,
                "VP8L predictor image byte size overflow",
            )
        })?;
    let mut rgba = Vec::new();
    rgba.try_reserve_exact(expected_bytes).map_err(|_| {
        DecodeError::new(
            DecodeErrorKind::AllocationFailed,
            None,
            "RGBA output allocation failed",
        )
    })?;

    for (y, residual_row) in pixels.chunks_exact(layout.width).enumerate() {
        for &pixel in residual_row {
            rgba.extend_from_slice(&unpack_rgba(pixel));
        }
        let row_start = y * layout.row_bytes;
        if y == 0 {
            let current = &mut rgba[..layout.row_bytes];
            current[3] = current[3].wrapping_add(255);
            for byte in 4..layout.row_bytes {
                current[byte] = current[byte].wrapping_add(current[byte - 4]);
            }
            continue;
        }

        let (previous_rows, current_row) = rgba.split_at_mut(row_start);
        let top = &previous_rows[row_start - layout.row_bytes..row_start];
        let current = &mut current_row[..layout.row_bytes];
        for channel in 0..4 {
            current[channel] = current[channel].wrapping_add(top[channel]);
        }
        let mode_row = (y / layout.block_size) * layout.mode_width;
        let mut x = 1;
        while x < layout.width {
            let block_x = x / layout.block_size;
            let mode =
                PredictorMode::try_from(((mode_pixels[mode_row + block_x] >> 8) & 0x0f) as u8)
                    .map_err(|_| {
                        DecodeError::new(
                            DecodeErrorKind::InvalidBitstream,
                            None,
                            "VP8L predictor mode must be in 0..=13",
                        )
                    })?;
            let x_end = (x & !(layout.block_size - 1))
                .saturating_add(layout.block_size)
                .min(layout.width);
            apply_predictor_run_rgba(current, top, x, x_end, mode);
            x = x_end;
        }
    }
    Ok(rgba)
}

/// Reconstructs residuals that are already stored in final RGBA byte order.
fn inverse_predictor_rgba(
    pixels: &mut [u8],
    descriptor: BlockTransformDescriptor,
    mode_pixels: &[u32],
) -> Result<(), DecodeError> {
    let layout = predictor_layout(descriptor, mode_pixels)?;
    let expected_bytes = layout.row_bytes.checked_mul(layout.height).ok_or_else(|| {
        DecodeError::new(
            DecodeErrorKind::LimitExceeded,
            None,
            "VP8L predictor image byte size overflow",
        )
    })?;
    if pixels.len() != expected_bytes {
        return Err(DecodeError::new(
            DecodeErrorKind::InvalidBitstream,
            None,
            "VP8L predictor output length does not match image dimensions",
        ));
    }

    // The first row has fixed black/left predictors independent of its mode
    // subimage. RGBA's alpha byte is the only nonzero black component.
    pixels[3] = pixels[3].wrapping_add(255);
    for byte in 4..layout.row_bytes {
        pixels[byte] = pixels[byte].wrapping_add(pixels[byte - 4]);
    }

    for y in 1..layout.height {
        let row_start = y * layout.row_bytes;
        let (previous_rows, current_and_after) = pixels.split_at_mut(row_start);
        let top = &previous_rows[row_start - layout.row_bytes..row_start];
        let current = &mut current_and_after[..layout.row_bytes];
        for channel in 0..4 {
            current[channel] = current[channel].wrapping_add(top[channel]);
        }

        let mode_row = (y / layout.block_size) * layout.mode_width;
        let mut x = 1;
        while x < layout.width {
            let block_x = x / layout.block_size;
            let mode =
                PredictorMode::try_from(((mode_pixels[mode_row + block_x] >> 8) & 0x0f) as u8)
                    .map_err(|_| {
                        DecodeError::new(
                            DecodeErrorKind::InvalidBitstream,
                            None,
                            "VP8L predictor mode must be in 0..=13",
                        )
                    })?;
            let x_end = (x & !(layout.block_size - 1))
                .saturating_add(layout.block_size)
                .min(layout.width);
            apply_predictor_run_rgba(current, top, x, x_end, mode);
            x = x_end;
        }
    }
    Ok(())
}

fn apply_predictor_run_rgba(
    current: &mut [u8],
    top: &[u8],
    start_x: usize,
    end_x: usize,
    mode: PredictorMode,
) {
    macro_rules! reconstruct {
        ($x:ident, $prediction:expr) => {
            for $x in start_x..end_x {
                let prediction = $prediction;
                add_rgba_pixel(current, $x, prediction);
            }
        };
    }

    match mode {
        PredictorMode::OpaqueBlack => {
            for pixel in current[start_x * 4..end_x * 4].chunks_exact_mut(4) {
                pixel[3] = pixel[3].wrapping_add(255);
            }
        }
        PredictorMode::Left => {
            for byte in start_x * 4..end_x * 4 {
                current[byte] = current[byte].wrapping_add(current[byte - 4]);
            }
        }
        PredictorMode::Top => add_aligned_rgba(&mut current[start_x * 4..end_x * 4], top, start_x),
        PredictorMode::TopLeft => {
            add_aligned_rgba(&mut current[start_x * 4..end_x * 4], top, start_x - 1);
        }
        PredictorMode::TopRight => {
            reconstruct!(x, top_right_rgba(current, top, x));
        }
        PredictorMode::AverageLeftTopRightTop => {
            reconstruct!(
                x,
                average_rgba(
                    average_rgba(rgba_pixel(current, x - 1), top_right_rgba(current, top, x)),
                    rgba_pixel(top, x),
                )
            );
        }
        PredictorMode::AverageLeftTopLeft => {
            reconstruct!(
                x,
                average_rgba(rgba_pixel(current, x - 1), rgba_pixel(top, x - 1))
            );
        }
        PredictorMode::AverageLeftTop => {
            reconstruct!(
                x,
                average_rgba(rgba_pixel(current, x - 1), rgba_pixel(top, x))
            );
        }
        PredictorMode::AverageTopLeftTop => {
            reconstruct!(x, average_rgba(rgba_pixel(top, x - 1), rgba_pixel(top, x)));
        }
        PredictorMode::AverageTopTopRight => {
            reconstruct!(
                x,
                average_rgba(rgba_pixel(top, x), top_right_rgba(current, top, x))
            );
        }
        PredictorMode::AverageLeftTopLeftTopTopRight => {
            reconstruct!(
                x,
                average_rgba(
                    average_rgba(rgba_pixel(current, x - 1), rgba_pixel(top, x - 1)),
                    average_rgba(rgba_pixel(top, x), top_right_rgba(current, top, x)),
                )
            );
        }
        PredictorMode::Select => apply_select_rgba(current, top, start_x, end_x),
        PredictorMode::ClampAddSubtractFull => {
            apply_clamped_add_subtract_full_rgba(current, top, start_x, end_x);
        }
        PredictorMode::ClampAddSubtractHalf => {
            reconstruct!(
                x,
                clamp_add_subtract_half_rgba(
                    rgba_pixel(current, x - 1),
                    rgba_pixel(top, x),
                    rgba_pixel(top, x - 1),
                )
            );
        }
    }
}

/// Reconstructs VP8L's select predictor over one mode run.
///
/// Select is the only predictor used by every method-0 CLIC stream. It has a
/// left-to-right dependency, so treating it as four independent byte slices
/// does not expose useful parallelism. Keeping the reconstructed left pixel
/// in a local value instead avoids reloading it and avoids constructing the
/// unused top-right neighbor required by the generic predictor adapter.
fn apply_select_rgba(current: &mut [u8], top: &[u8], start_x: usize, end_x: usize) {
    let byte_len = (end_x - start_x) * 4;
    let (reconstructed, residual_and_after) = current.split_at_mut(start_x * 4);
    let mut left: [u8; 4] = reconstructed[reconstructed.len() - 4..]
        .try_into()
        .expect("predictor run has a reconstructed left pixel");
    let residuals = &mut residual_and_after[..byte_len];
    let top_left = &top[(start_x - 1) * 4..][..byte_len];
    let top = &top[start_x * 4..][..byte_len];

    for ((residual, top_left), top) in residuals
        .chunks_exact_mut(4)
        .zip(top_left.chunks_exact(4))
        .zip(top.chunks_exact(4))
    {
        // For p = left + top - top_left, the distances to left and top
        // simplify to |top - top_left| and |left - top_left| respectively.
        // On a tie VP8L selects top.
        let top_distance = i16::from(top[0]).abs_diff(i16::from(top_left[0]))
            + i16::from(top[1]).abs_diff(i16::from(top_left[1]))
            + i16::from(top[2]).abs_diff(i16::from(top_left[2]))
            + i16::from(top[3]).abs_diff(i16::from(top_left[3]));
        let left_distance = i16::from(left[0]).abs_diff(i16::from(top_left[0]))
            + i16::from(left[1]).abs_diff(i16::from(top_left[1]))
            + i16::from(left[2]).abs_diff(i16::from(top_left[2]))
            + i16::from(left[3]).abs_diff(i16::from(top_left[3]));
        let prediction = if top_distance < left_distance {
            left
        } else {
            [top[0], top[1], top[2], top[3]]
        };
        left = [
            residual[0].wrapping_add(prediction[0]),
            residual[1].wrapping_add(prediction[1]),
            residual[2].wrapping_add(prediction[2]),
            residual[3].wrapping_add(prediction[3]),
        ];
        residual.copy_from_slice(&left);
    }
}

fn add_aligned_rgba(current: &mut [u8], top: &[u8], top_start_x: usize) {
    let top = &top[top_start_x * 4..top_start_x * 4 + current.len()];
    for (residual, &prediction) in current.iter_mut().zip(top) {
        *residual = residual.wrapping_add(prediction);
    }
}

fn apply_clamped_add_subtract_full_rgba(
    current: &mut [u8],
    top: &[u8],
    start_x: usize,
    end_x: usize,
) {
    let byte_len = (end_x - start_x) * 4;
    let (reconstructed, residual_and_after) = current.split_at_mut(start_x * 4);
    let mut left: [u8; 4] = reconstructed[reconstructed.len() - 4..]
        .try_into()
        .expect("predictor run has a reconstructed left pixel");
    let residuals = &mut residual_and_after[..byte_len];
    let top_left = &top[(start_x - 1) * 4..][..byte_len];
    let top = &top[start_x * 4..][..byte_len];

    for ((residual, top_left), top) in residuals
        .chunks_exact_mut(4)
        .zip(top_left.chunks_exact(4))
        .zip(top.chunks_exact(4))
    {
        left = [
            residual[0].wrapping_add(clamp_add_subtract_component(left[0], top[0], top_left[0])),
            residual[1].wrapping_add(clamp_add_subtract_component(left[1], top[1], top_left[1])),
            residual[2].wrapping_add(clamp_add_subtract_component(left[2], top[2], top_left[2])),
            residual[3].wrapping_add(clamp_add_subtract_component(left[3], top[3], top_left[3])),
        ];
        residual.copy_from_slice(&left);
    }
}

#[inline]
fn clamp_add_subtract_component(left: u8, top: u8, top_left: u8) -> u8 {
    (i16::from(left) + i16::from(top) - i16::from(top_left)).clamp(0, 255) as u8
}

#[inline]
fn rgba_pixel(pixels: &[u8], x: usize) -> [u8; 4] {
    let offset = x * 4;
    pixels[offset..offset + 4]
        .try_into()
        .expect("validated RGBA pixel")
}

#[inline]
fn top_right_rgba(current: &[u8], top: &[u8], x: usize) -> [u8; 4] {
    if x + 1 < top.len() / 4 {
        rgba_pixel(top, x + 1)
    } else {
        rgba_pixel(current, 0)
    }
}

#[inline]
fn add_rgba_pixel(pixels: &mut [u8], x: usize, prediction: [u8; 4]) {
    let offset = x * 4;
    for channel in 0..4 {
        pixels[offset + channel] = pixels[offset + channel].wrapping_add(prediction[channel]);
    }
}

#[inline]
fn average_rgba(first: [u8; 4], second: [u8; 4]) -> [u8; 4] {
    [
        (first[0] & second[0]).wrapping_add((first[0] ^ second[0]) >> 1),
        (first[1] & second[1]).wrapping_add((first[1] ^ second[1]) >> 1),
        (first[2] & second[2]).wrapping_add((first[2] ^ second[2]) >> 1),
        (first[3] & second[3]).wrapping_add((first[3] ^ second[3]) >> 1),
    ]
}

#[inline]
fn clamp_add_subtract_half_component(average: u8, top_left: u8) -> u8 {
    let average = i16::from(average);
    (average + (average - i16::from(top_left)) / 2).clamp(0, 255) as u8
}

#[inline]
fn clamp_add_subtract_half_rgba(left: [u8; 4], top: [u8; 4], top_left: [u8; 4]) -> [u8; 4] {
    let average = average_rgba(left, top);
    [
        clamp_add_subtract_half_component(average[0], top_left[0]),
        clamp_add_subtract_half_component(average[1], top_left[1]),
        clamp_add_subtract_half_component(average[2], top_left[2]),
        clamp_add_subtract_half_component(average[3], top_left[3]),
    ]
}

/// Test-only packed reference used to validate the production RGBA predictor.
///
/// Keeping this structurally independent makes the fourteen-mode differential
/// test useful; it is not selected or compiled into the production decoder.
#[cfg(test)]
fn inverse_predictor_argb_reference(
    pixels: &mut [u32],
    descriptor: BlockTransformDescriptor,
    mode_pixels: &[u32],
) -> Result<(), DecodeError> {
    let width = usize::try_from(descriptor.image_width).map_err(|_| {
        DecodeError::new(
            DecodeErrorKind::LimitExceeded,
            None,
            "VP8L predictor image width does not fit usize",
        )
    })?;
    let height = usize::try_from(descriptor.image_height).map_err(|_| {
        DecodeError::new(
            DecodeErrorKind::LimitExceeded,
            None,
            "VP8L predictor image height does not fit usize",
        )
    })?;
    let expected_pixels = width.checked_mul(height).ok_or_else(|| {
        DecodeError::new(
            DecodeErrorKind::LimitExceeded,
            None,
            "VP8L predictor image pixel count overflow",
        )
    })?;
    if pixels.len() != expected_pixels {
        return Err(DecodeError::new(
            DecodeErrorKind::InvalidBitstream,
            None,
            "VP8L predictor output length does not match image dimensions",
        ));
    }
    let mode_width = usize::try_from(descriptor.transform_width).map_err(|_| {
        DecodeError::new(
            DecodeErrorKind::LimitExceeded,
            None,
            "VP8L predictor mode width does not fit usize",
        )
    })?;
    let expected_modes = mode_width
        .checked_mul(usize::try_from(descriptor.transform_height).map_err(|_| {
            DecodeError::new(
                DecodeErrorKind::LimitExceeded,
                None,
                "VP8L predictor mode height does not fit usize",
            )
        })?)
        .ok_or_else(|| {
            DecodeError::new(
                DecodeErrorKind::LimitExceeded,
                None,
                "VP8L predictor mode pixel count overflow",
            )
        })?;
    if mode_pixels.len() != expected_modes {
        return Err(DecodeError::new(
            DecodeErrorKind::InvalidBitstream,
            None,
            "VP8L predictor mode image has unexpected dimensions",
        ));
    }

    let block_size = usize::try_from(descriptor.block_size()).map_err(|_| {
        DecodeError::new(
            DecodeErrorKind::LimitExceeded,
            None,
            "VP8L predictor block size does not fit usize",
        )
    })?;

    // VP8L fixes the top-left predictor to opaque black and the remainder of
    // the first row to the reconstructed pixel on the left.
    pixels[0] = add_argb_pixels(pixels[0], 0xff00_0000);
    for x in 1..width {
        pixels[x] = add_argb_pixels(pixels[x], pixels[x - 1]);
    }

    for y in 1..height {
        let row_start = y * width;
        // The first pixel of every later row always predicts from above.
        pixels[row_start] = add_argb_pixels(pixels[row_start], pixels[row_start - width]);

        let mode_row = (y / block_size) * mode_width;
        let mut x = 1;
        while x < width {
            let block_x = x / block_size;
            let mode =
                PredictorMode::try_from(((mode_pixels[mode_row + block_x] >> 8) & 0x0f) as u8)
                    .map_err(|_| {
                        DecodeError::new(
                            DecodeErrorKind::InvalidBitstream,
                            None,
                            "VP8L predictor mode must be in 0..=13",
                        )
                    })?;
            let x_end = (x & !(block_size - 1))
                .saturating_add(block_size)
                .min(width);
            apply_predictor_run(pixels, row_start + x, row_start + x_end, width, mode);
            x = x_end;
        }
    }
    Ok(())
}

#[cfg(test)]
fn apply_predictor_run(
    pixels: &mut [u32],
    start: usize,
    end: usize,
    width: usize,
    mode: PredictorMode,
) {
    macro_rules! reconstruct {
        ($offset:ident, $prediction:expr) => {
            for $offset in start..end {
                pixels[$offset] = add_argb_pixels(pixels[$offset], $prediction);
            }
        };
    }

    match mode {
        PredictorMode::OpaqueBlack => reconstruct!(offset, 0xff00_0000),
        PredictorMode::Left => reconstruct!(offset, pixels[offset - 1]),
        PredictorMode::Top => reconstruct!(offset, pixels[offset - width]),
        PredictorMode::TopRight => reconstruct!(offset, pixels[offset + 1 - width]),
        PredictorMode::TopLeft => reconstruct!(offset, pixels[offset - 1 - width]),
        PredictorMode::AverageLeftTopRightTop => reconstruct!(
            offset,
            average_argb3(
                pixels[offset - 1],
                pixels[offset + 1 - width],
                pixels[offset - width],
            )
        ),
        PredictorMode::AverageLeftTopLeft => reconstruct!(
            offset,
            average_argb2(pixels[offset - 1], pixels[offset - 1 - width])
        ),
        PredictorMode::AverageLeftTop => reconstruct!(
            offset,
            average_argb2(pixels[offset - 1], pixels[offset - width])
        ),
        PredictorMode::AverageTopLeftTop => reconstruct!(
            offset,
            average_argb2(pixels[offset - 1 - width], pixels[offset - width])
        ),
        PredictorMode::AverageTopTopRight => reconstruct!(
            offset,
            average_argb2(pixels[offset - width], pixels[offset + 1 - width])
        ),
        PredictorMode::AverageLeftTopLeftTopTopRight => reconstruct!(
            offset,
            average_argb4(
                pixels[offset - 1],
                pixels[offset - 1 - width],
                pixels[offset - width],
                pixels[offset + 1 - width],
            )
        ),
        PredictorMode::Select => reconstruct!(
            offset,
            select_argb(
                pixels[offset - 1],
                pixels[offset - width],
                pixels[offset - 1 - width],
            )
        ),
        PredictorMode::ClampAddSubtractFull => reconstruct!(
            offset,
            clamp_add_subtract_full_argb(
                pixels[offset - 1],
                pixels[offset - width],
                pixels[offset - 1 - width],
            )
        ),
        PredictorMode::ClampAddSubtractHalf => reconstruct!(
            offset,
            clamp_add_subtract_half_argb(
                pixels[offset - 1],
                pixels[offset - width],
                pixels[offset - 1 - width],
            )
        ),
    }
}

#[cfg(test)]
#[inline]
fn add_argb_pixels(first: u32, second: u32) -> u32 {
    const ALPHA_GREEN: u32 = 0xff00_ff00;
    const RED_BLUE: u32 = 0x00ff_00ff;
    let alpha_green = (first & ALPHA_GREEN).wrapping_add(second & ALPHA_GREEN);
    let red_blue = (first & RED_BLUE).wrapping_add(second & RED_BLUE);
    (alpha_green & ALPHA_GREEN) | (red_blue & RED_BLUE)
}

#[cfg(test)]
#[inline]
fn average_argb2(first: u32, second: u32) -> u32 {
    (((first ^ second) & 0xfefe_fefe) >> 1).wrapping_add(first & second)
}

#[cfg(test)]
#[inline]
fn average_argb3(first: u32, second: u32, third: u32) -> u32 {
    average_argb2(average_argb2(first, second), third)
}

#[cfg(test)]
#[inline]
fn average_argb4(first: u32, second: u32, third: u32, fourth: u32) -> u32 {
    average_argb2(average_argb2(first, second), average_argb2(third, fourth))
}

#[cfg(test)]
#[inline]
fn select_argb(left: u32, top: u32, top_left: u32) -> u32 {
    let distance_difference = select_component(top >> 24, left >> 24, top_left >> 24)
        + select_component(
            (top >> 16) & 0xff,
            (left >> 16) & 0xff,
            (top_left >> 16) & 0xff,
        )
        + select_component(
            (top >> 8) & 0xff,
            (left >> 8) & 0xff,
            (top_left >> 8) & 0xff,
        )
        + select_component(top & 0xff, left & 0xff, top_left & 0xff);
    if distance_difference <= 0 { top } else { left }
}

#[cfg(test)]
#[inline]
fn select_component(first: u32, second: u32, reference: u32) -> i32 {
    let first = first as i32 - reference as i32;
    let second = second as i32 - reference as i32;
    second.abs() - first.abs()
}

#[cfg(test)]
fn clamp_add_subtract_full_argb(first: u32, second: u32, third: u32) -> u32 {
    pack_argb_components(|shift| {
        component(first, shift) + component(second, shift) - component(third, shift)
    })
}

#[cfg(test)]
fn clamp_add_subtract_half_argb(first: u32, second: u32, third: u32) -> u32 {
    let average = average_argb2(first, second);
    pack_argb_components(|shift| {
        let value = component(average, shift);
        value + (value - component(third, shift)) / 2
    })
}

#[cfg(test)]
#[inline]
fn component(pixel: u32, shift: u32) -> i32 {
    ((pixel >> shift) & 0xff) as i32
}

#[cfg(test)]
fn pack_argb_components(mut value_at: impl FnMut(u32) -> i32) -> u32 {
    let blue = value_at(0).clamp(0, 255) as u32;
    let green = value_at(8).clamp(0, 255) as u32;
    let red = value_at(16).clamp(0, 255) as u32;
    let alpha = value_at(24).clamp(0, 255) as u32;
    (alpha << 24) | (red << 16) | (green << 8) | blue
}

const fn argb_to_rgba(pixel: u32) -> Rgba {
    Rgba::new(
        (pixel >> 16) as u8,
        (pixel >> 8) as u8,
        pixel as u8,
        (pixel >> 24) as u8,
    )
}

struct HuffmanCodes {
    green: FastHuffmanTable,
    red: FastHuffmanTable,
    blue: FastHuffmanTable,
    alpha: FastHuffmanTable,
    distance: FastHuffmanTable,
}

/// The maximum number of prefix tables in one VP8L meta-prefix group.
const HUFFMAN_TABLES_PER_GROUP: usize = 5;

// `HuffmanTable` is intentionally opaque to this crate. Reserve a deliberately
// conservative amount for every possible wire symbol so the allocation limit
// also covers the heap storage hidden behind its vectors. The root lookup
// table is a fixed heap allocation per table and is accounted separately.
const MAX_HUFFMAN_CODE_STORAGE_BYTES: usize = 64;

enum EntropyCodes {
    // Keep the five-table single group off the enum's stack representation.
    // A one-element boxed slice lets construction report allocation failure
    // through Vec::try_reserve_exact rather than aborting via Box::new.
    Single(Box<[HuffmanCodes]>),
    Meta(MetaHuffmanCodes),
}

impl EntropyCodes {
    fn cursor(&self, image_width: u32) -> Result<EntropyCodeCursor<'_>, DecodeError> {
        EntropyCodeCursor::new(self, image_width)
    }
}

/// Selects one maximal horizontal run that shares a meta-Huffman group.
enum EntropyCodeCursor<'a> {
    Single(&'a HuffmanCodes),
    Meta {
        codes: &'a MetaHuffmanCodes,
        image_width: usize,
        pixel: usize,
        x: usize,
        y: usize,
    },
}

impl<'a> EntropyCodeCursor<'a> {
    fn new(codes: &'a EntropyCodes, image_width: u32) -> Result<Self, DecodeError> {
        match codes {
            EntropyCodes::Single(codes) => codes.first().map(Self::Single).ok_or_else(|| {
                DecodeError::new(
                    DecodeErrorKind::InvalidBitstream,
                    None,
                    "VP8L single Huffman-code group is missing",
                )
            }),
            EntropyCodes::Meta(codes) => Ok(Self::Meta {
                codes,
                image_width: usize::try_from(image_width).map_err(|_| {
                    DecodeError::new(
                        DecodeErrorKind::LimitExceeded,
                        None,
                        "VP8L image width does not fit usize",
                    )
                })?,
                pixel: 0,
                x: 0,
                y: 0,
            }),
        }
    }

    fn run_for_pixel(
        &mut self,
        pixel: usize,
        pixel_limit: usize,
    ) -> Result<(&'a HuffmanCodes, usize), DecodeError> {
        match self {
            Self::Single(codes) => Ok((codes, pixel_limit)),
            Self::Meta {
                codes,
                image_width,
                pixel: cursor_pixel,
                x,
                y,
            } => {
                let advance = pixel.checked_sub(*cursor_pixel).ok_or_else(|| {
                    DecodeError::new(
                        DecodeErrorKind::InvalidBitstream,
                        None,
                        "VP8L meta-prefix group cursor moved backward",
                    )
                })?;
                let row_remaining = *image_width - *x;
                if advance < row_remaining {
                    *x += advance;
                } else if advance == row_remaining {
                    *x = 0;
                    *y += 1;
                } else {
                    let following_rows = advance - row_remaining;
                    *y += 1 + following_rows / *image_width;
                    *x = following_rows % *image_width;
                }
                *cursor_pixel = pixel;

                let block_size = 1_usize << codes.prefix_bits;
                let block_x = *x >> codes.prefix_bits;
                let block_y = *y >> codes.prefix_bits;
                let map_index = block_y
                    .checked_mul(codes.prefix_image_width)
                    .and_then(|value| value.checked_add(block_x))
                    .ok_or_else(|| {
                        DecodeError::new(
                            DecodeErrorKind::InvalidBitstream,
                            None,
                            "VP8L meta-prefix image index overflow",
                        )
                    })?;
                let group_index = usize::from(*codes.group_map.get(map_index).ok_or_else(|| {
                    DecodeError::new(
                        DecodeErrorKind::InvalidBitstream,
                        None,
                        "VP8L meta-prefix image does not cover output pixel",
                    )
                })?);
                let group = codes.groups.get(group_index).ok_or_else(|| {
                    DecodeError::new(
                        DecodeErrorKind::InvalidBitstream,
                        None,
                        "VP8L meta-prefix group table is missing",
                    )
                })?;

                let run_in_block = block_size - (*x & (block_size - 1));
                let run_in_row = *image_width - *x;
                let run_end = pixel
                    .checked_add(run_in_block.min(run_in_row))
                    .ok_or_else(|| {
                        DecodeError::new(
                            DecodeErrorKind::InvalidBitstream,
                            None,
                            "VP8L meta-prefix group cursor overflow",
                        )
                    })?
                    .min(pixel_limit);
                Ok((group, run_end))
            }
        }
    }

    #[cfg(test)]
    const fn is_meta(&self) -> bool {
        matches!(self, Self::Meta { .. })
    }
}

fn box_huffman_codes(codes: HuffmanCodes) -> Result<Box<[HuffmanCodes]>, DecodeError> {
    let mut boxed = Vec::new();
    boxed.try_reserve_exact(1).map_err(|_| {
        DecodeError::new(
            DecodeErrorKind::AllocationFailed,
            None,
            "VP8L single Huffman-code group allocation failed",
        )
    })?;
    boxed.push(codes);
    Ok(boxed.into_boxed_slice())
}

/// A decoded meta-prefix image and the prefix-code groups it references.
///
/// VP8L writes groups for every numeric id through the largest id in the
/// entropy image.  We parse all of those groups to preserve stream alignment,
/// but retain only the distinct ids that are actually selected by a pixel.
/// This is important for valid streams with sparse, high-valued ids.
struct MetaHuffmanCodes {
    prefix_bits: u8,
    prefix_image_width: usize,
    /// Dense indices into `groups`, remapped once from the sparse wire ids.
    group_map: Vec<u16>,
    groups: Vec<HuffmanCodes>,
}

#[allow(clippy::too_many_arguments)]
fn read_meta_huffman_codes(
    bits: &mut BitReader<'_>,
    width: u32,
    height: u32,
    color_cache_size: usize,
    budget: &mut WorkBudget,
    limits: &DecodeLimits,
    retained_bytes: usize,
    final_rgba_bytes: usize,
) -> Result<MetaHuffmanCodes, DecodeError> {
    budget.consume(1)?;
    let prefix_bits = bits.read_bits(3)? as u8 + 2;
    let (prefix_image_width, prefix_image_height) =
        prefix_image_dimensions(width, height, prefix_bits)?;
    let entropy_image = decode_entropy_image(
        bits,
        prefix_image_width,
        prefix_image_height,
        false,
        budget,
        limits,
        retained_bytes,
        0,
    )?;

    let entropy_image_bytes = checked_transform_bytes(
        entropy_image.len(),
        size_of::<u32>(),
        "VP8L meta-prefix entropy image byte size overflow",
    )?;
    let group_map_bytes = checked_transform_bytes(
        entropy_image.len(),
        size_of::<u16>(),
        "VP8L meta-prefix group map byte size overflow",
    )?;
    check_meta_conversion_allocation(
        retained_bytes,
        entropy_image_bytes,
        group_map_bytes,
        limits.max_alloc_bytes,
    )?;

    let mut group_map = Vec::new();
    group_map
        .try_reserve_exact(entropy_image.len())
        .map_err(|_| {
            DecodeError::new(
                DecodeErrorKind::AllocationFailed,
                None,
                "VP8L meta-prefix group map allocation failed",
            )
        })?;
    for pixel in entropy_image {
        // VP8L stores the 16-bit id in the red and green bytes of the ARGB
        // entropy-image pixel, with green as the low byte.
        group_map.push(((pixel >> 8) & 0xffff) as u16);
    }

    check_meta_group_id_collection_allocation(
        retained_bytes,
        group_map_bytes,
        limits.max_alloc_bytes,
    )?;
    let mut group_ids = Vec::new();
    group_ids.try_reserve_exact(group_map.len()).map_err(|_| {
        DecodeError::new(
            DecodeErrorKind::AllocationFailed,
            None,
            "VP8L meta-prefix group id allocation failed",
        )
    })?;
    group_ids.extend_from_slice(&group_map);
    group_ids.sort_unstable();
    group_ids.dedup();
    let max_group_id = usize::from(*group_ids.last().ok_or_else(|| {
        DecodeError::new(
            DecodeErrorKind::InvalidBitstream,
            None,
            "VP8L meta-prefix image is empty",
        )
    })?);
    let declared_groups = max_group_id.checked_add(1).ok_or_else(|| {
        DecodeError::new(
            DecodeErrorKind::LimitExceeded,
            None,
            "VP8L meta-prefix group count overflow",
        )
    })?;
    let _declared_table_count = declared_groups
        .checked_mul(HUFFMAN_TABLES_PER_GROUP)
        .ok_or_else(|| {
            DecodeError::new(
                DecodeErrorKind::LimitExceeded,
                None,
                "VP8L meta-prefix table count overflow",
            )
        })?;

    let group_storage = meta_group_storage_upper_bound(color_cache_size)?;
    let retained_group_storage = group_storage.checked_mul(group_ids.len()).ok_or_else(|| {
        DecodeError::new(
            DecodeErrorKind::LimitExceeded,
            None,
            "VP8L meta-prefix retained table storage overflow",
        )
    })?;
    // While an unused group is being parsed it still owns its five table
    // vectors briefly.  Account for that transient allocation too.
    let meta_retained = retained_bytes
        .checked_add(group_map_bytes)
        .and_then(|value| value.checked_add(group_map_bytes))
        .and_then(|value| value.checked_add(retained_group_storage))
        .and_then(|value| value.checked_add(group_storage))
        .ok_or_else(|| {
            DecodeError::new(
                DecodeErrorKind::LimitExceeded,
                None,
                "VP8L meta-prefix storage accounting overflow",
            )
        })?;
    let pixels = pixel_count(width, height)?;
    check_allocation_budget(
        pixels,
        final_rgba_bytes,
        color_cache_size,
        meta_retained,
        limits.max_alloc_bytes,
    )?;

    let mut groups = Vec::new();
    groups.try_reserve_exact(group_ids.len()).map_err(|_| {
        DecodeError::new(
            DecodeErrorKind::AllocationFailed,
            None,
            "VP8L meta-prefix group storage allocation failed",
        )
    })?;
    let mut next_used_group = 0_usize;
    for group_id in 0..declared_groups {
        let codes = read_huffman_codes(bits, budget, color_cache_size)?;
        if group_ids
            .get(next_used_group)
            .is_some_and(|&id| usize::from(id) == group_id)
        {
            groups.push(codes);
            next_used_group += 1;
        }
    }
    debug_assert_eq!(next_used_group, group_ids.len());

    for group in &mut group_map {
        let group_index = group_ids.binary_search(group).map_err(|_| {
            DecodeError::new(
                DecodeErrorKind::InvalidBitstream,
                None,
                "VP8L meta-prefix group was not retained",
            )
        })?;
        *group = u16::try_from(group_index).map_err(|_| {
            DecodeError::new(
                DecodeErrorKind::LimitExceeded,
                None,
                "VP8L meta-prefix dense group index exceeds u16",
            )
        })?;
    }

    Ok(MetaHuffmanCodes {
        prefix_bits,
        prefix_image_width: usize::try_from(prefix_image_width).map_err(|_| {
            DecodeError::new(
                DecodeErrorKind::LimitExceeded,
                None,
                "VP8L meta-prefix image width does not fit usize",
            )
        })?,
        group_map,
        groups,
    })
}

fn prefix_image_dimensions(
    width: u32,
    height: u32,
    prefix_bits: u8,
) -> Result<(u32, u32), DecodeError> {
    if !(2..=9).contains(&prefix_bits) {
        return Err(DecodeError::new(
            DecodeErrorKind::InvalidBitstream,
            None,
            "VP8L meta-prefix bits must be in 2..=9",
        ));
    }
    let block_size = 1_u32 << prefix_bits;
    Ok((width.div_ceil(block_size), height.div_ceil(block_size)))
}

fn meta_group_storage_upper_bound(color_cache_size: usize) -> Result<usize, DecodeError> {
    let green_alphabet = GREEN_ALPHABET_SIZE
        .checked_add(color_cache_size)
        .ok_or_else(|| {
            DecodeError::new(
                DecodeErrorKind::LimitExceeded,
                None,
                "VP8L meta-prefix green alphabet size overflow",
            )
        })?;
    let symbol_count = green_alphabet
        .checked_add(CHANNEL_ALPHABET_SIZE.checked_mul(3).ok_or_else(|| {
            DecodeError::new(
                DecodeErrorKind::LimitExceeded,
                None,
                "VP8L meta-prefix channel alphabet size overflow",
            )
        })?)
        .and_then(|value| value.checked_add(DISTANCE_ALPHABET_SIZE))
        .ok_or_else(|| {
            DecodeError::new(
                DecodeErrorKind::LimitExceeded,
                None,
                "VP8L meta-prefix symbol count overflow",
            )
        })?;
    symbol_count
        .checked_mul(MAX_HUFFMAN_CODE_STORAGE_BYTES)
        .and_then(|value| {
            value.checked_add(HUFFMAN_TABLES_PER_GROUP * size_of::<FastHuffmanTable>())
        })
        // A normal code header has one transient root table while its final
        // table is being built. Include that extra allocation per group as a
        // conservative bound for both retained and skipped meta groups.
        .and_then(|value| {
            value.checked_add(
                (HUFFMAN_TABLES_PER_GROUP + 1)
                    * (ROOT_TABLE_STORAGE_BYTES + MAX_SECONDARY_TABLE_STORAGE_BYTES),
            )
        })
        .ok_or_else(|| {
            DecodeError::new(
                DecodeErrorKind::LimitExceeded,
                None,
                "VP8L meta-prefix group storage overflow",
            )
        })
}

fn check_meta_conversion_allocation(
    retained_bytes: usize,
    entropy_image_bytes: usize,
    group_map_bytes: usize,
    max_alloc_bytes: usize,
) -> Result<(), DecodeError> {
    let total = retained_bytes
        .checked_add(entropy_image_bytes)
        .and_then(|value| value.checked_add(group_map_bytes))
        .ok_or_else(|| {
            DecodeError::new(
                DecodeErrorKind::LimitExceeded,
                None,
                "VP8L meta-prefix conversion allocation size overflow",
            )
        })?;
    if total > max_alloc_bytes {
        return Err(DecodeError::new(
            DecodeErrorKind::LimitExceeded,
            None,
            "VP8L meta-prefix conversion exceeds allocation limit",
        ));
    }
    Ok(())
}

fn check_meta_group_id_collection_allocation(
    retained_bytes: usize,
    group_map_bytes: usize,
    max_alloc_bytes: usize,
) -> Result<(), DecodeError> {
    let total = retained_bytes
        .checked_add(group_map_bytes)
        .and_then(|value| value.checked_add(group_map_bytes))
        .ok_or_else(|| {
            DecodeError::new(
                DecodeErrorKind::LimitExceeded,
                None,
                "VP8L meta-prefix group id allocation size overflow",
            )
        })?;
    if total > max_alloc_bytes {
        return Err(DecodeError::new(
            DecodeErrorKind::LimitExceeded,
            None,
            "VP8L meta-prefix group id collection exceeds allocation limit",
        ));
    }
    Ok(())
}

fn read_huffman_codes(
    bits: &mut BitReader<'_>,
    budget: &mut WorkBudget,
    color_cache_size: usize,
) -> Result<HuffmanCodes, DecodeError> {
    Ok(HuffmanCodes {
        green: read_table(
            bits,
            budget,
            GREEN_ALPHABET_SIZE
                .checked_add(color_cache_size)
                .ok_or_else(|| {
                    DecodeError::new(
                        DecodeErrorKind::InvalidBitstream,
                        None,
                        "VP8L color-cache alphabet size overflow",
                    )
                })?,
        )?,
        red: read_table(bits, budget, CHANNEL_ALPHABET_SIZE)?,
        blue: read_table(bits, budget, CHANNEL_ALPHABET_SIZE)?,
        alpha: read_table(bits, budget, CHANNEL_ALPHABET_SIZE)?,
        distance: read_table(bits, budget, DISTANCE_ALPHABET_SIZE)?,
    })
}

struct PixelOutput {
    pixels: Vec<u32>,
    cache: Option<DeferredColorCache>,
}

struct DeferredColorCache {
    cache: ColorCache,
    cached_pixels: usize,
}

impl PixelOutput {
    fn new(color_cache_bits: Option<u8>, pixels: usize) -> Result<Self, DecodeError> {
        let mut output = Vec::new();
        output.try_reserve_exact(pixels).map_err(|_| {
            DecodeError::new(
                DecodeErrorKind::AllocationFailed,
                None,
                "packed VP8L output allocation failed",
            )
        })?;
        let cache = color_cache_bits
            .map(|bits| {
                Ok(DeferredColorCache {
                    cache: ColorCache::new(bits)?,
                    cached_pixels: 0,
                })
            })
            .transpose()?;
        Ok(Self {
            pixels: output,
            cache,
        })
    }

    fn len(&self) -> usize {
        self.pixels.len()
    }

    fn emit_literal(&mut self, color: u32) -> Result<(), DecodeError> {
        // `PixelOutput::new` reserved the complete, already validated image
        // size. The enclosing entropy loop cannot emit past that size, so
        // this push never grows the vector. Cache population is deferred
        // until a cache symbol actually needs the state.
        self.pixels.push(color);
        Ok(())
    }

    fn emit_cache_hit(&mut self, index: usize) -> Result<(), DecodeError> {
        let deferred = self.cache.as_mut().ok_or_else(|| {
            DecodeError::new(
                DecodeErrorKind::InvalidBitstream,
                None,
                "VP8L color-cache symbol appeared without a color cache",
            )
        })?;
        for &color in &self.pixels[deferred.cached_pixels..] {
            deferred.cache.insert(color);
        }
        deferred.cached_pixels = self.pixels.len();
        let color = deferred.cache.get(index)?;
        self.pixels.push(color);
        Ok(())
    }

    fn copy_lz77(
        &mut self,
        length: usize,
        distance: usize,
        output_limit: usize,
        budget: &mut WorkBudget,
    ) -> Result<(), DecodeError> {
        copy_lz77_pixels_preallocated(
            &mut self.pixels,
            length,
            distance,
            output_limit,
            budget,
        )
    }

    fn into_pixels(self) -> Vec<u32> {
        self.pixels
    }
}

fn read_table(
    bits: &mut BitReader<'_>,
    budget: &mut WorkBudget,
    alphabet_size: usize,
) -> Result<FastHuffmanTable, DecodeError> {
    budget.consume(1)?;
    read_huffman_code(bits, alphabet_size)?.into_fast()
}

fn decode_fast_symbol(
    table: &FastHuffmanTable,
    bits: &mut webp_core::ShiftedBitReader<'_, '_>,
    budget: &mut WorkBudget,
) -> Result<usize, DecodeError> {
    budget.consume(1)?;
    if bits.available_bits() < 15 {
        bits.fill();
    }
    table.decode(bits).map(usize::from)
}

enum GreenOrLiteral {
    Green(usize),
    Literal(u32),
}

/// Decodes the four literal channels from one immutable bit-register snapshot
/// when every table has a packed representation. This removes three reader
/// state transitions and three repeated EOF checks from the dominant VP8L
/// path. Non-literals, rare fallback tables, and short tails retain the strict
/// per-symbol decoder.
#[inline]
fn decode_green_or_literal(
    codes: &HuffmanCodes,
    bits: &mut webp_core::ShiftedBitReader<'_, '_>,
    budget: &mut WorkBudget,
) -> Result<GreenOrLiteral, DecodeError> {
    budget.consume(1)?;
    if bits.available_bits() < 15 {
        bits.fill();
    }

    let lookahead = bits.peek_full();
    if let Some((green, green_bits)) = codes.green.lookup_buffered(lookahead as u16)
        && usize::from(green) < CHANNEL_ALPHABET_SIZE
    {
        let shifted = lookahead >> green_bits;
        if let Some((red, red_bits)) = codes.red.lookup_buffered(shifted as u16) {
            let used = green_bits + red_bits;
            let shifted = lookahead >> used;
            if let Some((blue, blue_bits)) = codes.blue.lookup_buffered(shifted as u16) {
                let used = used + blue_bits;
                let shifted = lookahead >> used;
                if let Some((alpha, alpha_bits)) = codes.alpha.lookup_buffered(shifted as u16) {
                    let used = used + alpha_bits;
                    if used <= bits.available_bits() {
                        budget.consume(3)?;
                        bits.consume_buffered(used)?;
                        return Ok(GreenOrLiteral::Literal(pack_argb(
                            red as u8,
                            green as u8,
                            blue as u8,
                            alpha as u8,
                        )));
                    }
                }
            }
        }
    }

    codes
        .green
        .decode(bits)
        .map(|symbol| GreenOrLiteral::Green(usize::from(symbol)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use webp_core::BitWriter;
    use webp_vp8l::SIGNATURE;

    fn limits() -> DecodeLimits {
        DecodeLimits::default()
    }

    fn write_header(writer: &mut BitWriter, width: u32, height: u32, alpha: bool) {
        writer.write_bits(u32::from(SIGNATURE), 8).unwrap();
        writer.write_bits(width - 1, 14).unwrap();
        writer.write_bits(height - 1, 14).unwrap();
        writer.write_bits(u32::from(alpha), 1).unwrap();
        writer.write_bits(0, 3).unwrap();
    }

    fn write_simple_code(writer: &mut BitWriter, symbol: u8) {
        writer.write_bits(1, 1).unwrap(); // simple_code_flag
        writer.write_bits(0, 1).unwrap(); // one symbol
        writer.write_bits(1, 1).unwrap(); // first symbol uses eight bits
        writer.write_bits(u32::from(symbol), 8).unwrap();
    }

    fn write_two_symbol_normal_code(
        writer: &mut BitWriter,
        alphabet_size: usize,
        first_symbol: usize,
        second_symbol: usize,
    ) {
        assert!(first_symbol < second_symbol);
        assert!(second_symbol < alphabet_size);
        writer.write_bits(0, 1).unwrap(); // normal_code_flag
        writer.write_bits(0, 4).unwrap(); // four code-length alphabet entries

        // Wire order is 17, 18, 0, 1. Symbols zero and one form a complete
        // code-length tree, so the following code lengths use one bit each.
        for length in [0_u32, 0, 1, 1] {
            writer.write_bits(length, 3).unwrap();
        }
        writer.write_bits(0, 1).unwrap(); // use_length = false
        for symbol in 0..alphabet_size {
            writer
                .write_bits(
                    u32::from(symbol == first_symbol || symbol == second_symbol),
                    1,
                )
                .unwrap();
        }
    }

    fn wire_code(lengths: &[u8], wanted_symbol: usize) -> (u32, u8) {
        let mut sorted: Vec<(u8, usize)> = lengths
            .iter()
            .copied()
            .enumerate()
            .filter_map(|(symbol, length)| (length != 0).then_some((length, symbol)))
            .collect();
        sorted.sort_unstable();

        let mut code = 0_u32;
        let mut previous_length = 0_u8;
        for (length, symbol) in sorted {
            code <<= u32::from(length - previous_length);
            if symbol == wanted_symbol {
                return (
                    code.reverse_bits() >> (u32::BITS - u32::from(length)),
                    length,
                );
            }
            code += 1;
            previous_length = length;
        }
        panic!("requested unused Huffman symbol");
    }

    fn write_normal_code(
        writer: &mut BitWriter,
        alphabet_size: usize,
        entries: &[(usize, u8)],
    ) -> Vec<u8> {
        let mut lengths = vec![0_u8; alphabet_size];
        for &(symbol, length) in entries {
            assert!(symbol < alphabet_size);
            lengths[symbol] = length;
        }

        writer.write_bits(0, 1).unwrap(); // normal_code_flag

        // Code-length symbols 0, 1, 2 and 3 all have two-bit codes. This
        // lets the fixture express the small complete trees used below.
        writer.write_bits(2, 4).unwrap(); // 4 + 2 == 6 entries
        for length in [0_u32, 0, 2, 2, 2, 2] {
            writer.write_bits(length, 3).unwrap();
        }
        writer.write_bits(0, 1).unwrap(); // use_length = false
        let code_length_lengths = [2_u8; 4];
        for &length in &lengths {
            let (code, width) = wire_code(&code_length_lengths, usize::from(length));
            writer.write_bits(code, width).unwrap();
        }
        lengths
    }

    fn write_symbol(writer: &mut BitWriter, lengths: &[u8], symbol: usize) {
        let (code, width) = wire_code(lengths, symbol);
        writer.write_bits(code, width).unwrap();
    }

    fn literal_stream(width: u32, height: u32, pixel: [u8; 4]) -> Vec<u8> {
        literal_stream_with_transforms(width, height, pixel, &[])
    }

    fn literal_stream_with_transforms(
        width: u32,
        height: u32,
        pixel: [u8; 4],
        transforms: &[u8],
    ) -> Vec<u8> {
        let mut writer = BitWriter::new();
        write_header(&mut writer, width, height, pixel[3] != 255);
        for &transform_type in transforms {
            writer.write_bits(1, 1).unwrap(); // transform_present
            writer.write_bits(u32::from(transform_type), 2).unwrap();
        }
        writer.write_bits(0, 1).unwrap(); // transform list terminator
        writer.write_bits(0, 1).unwrap(); // color_cache_present
        writer.write_bits(0, 1).unwrap(); // use_meta_huffman
        for symbol in [pixel[1], pixel[0], pixel[2], pixel[3], 0] {
            write_simple_code(&mut writer, symbol);
        }
        writer.into_bytes()
    }

    fn write_flat_entropy_image(writer: &mut BitWriter, pixel: [u8; 4], is_level0: bool) {
        writer.write_bits(0, 1).unwrap(); // color_cache_present
        if is_level0 {
            writer.write_bits(0, 1).unwrap(); // use_meta_huffman
        }
        for symbol in [pixel[1], pixel[0], pixel[2], pixel[3], 0] {
            write_simple_code(writer, symbol);
        }
    }

    fn write_channel_code(
        writer: &mut BitWriter,
        alphabet_size: usize,
        values: &[u8],
    ) -> Option<Vec<u8>> {
        let mut symbols = values.to_vec();
        symbols.sort_unstable();
        symbols.dedup();
        match symbols.len() {
            1 => {
                write_simple_code(writer, symbols[0]);
                None
            }
            2 => Some(write_normal_code(
                writer,
                alphabet_size,
                &[(usize::from(symbols[0]), 1), (usize::from(symbols[1]), 1)],
            )),
            3 => Some(write_normal_code(
                writer,
                alphabet_size,
                &[
                    (usize::from(symbols[0]), 1),
                    (usize::from(symbols[1]), 2),
                    (usize::from(symbols[2]), 2),
                ],
            )),
            4 => Some(write_normal_code(
                writer,
                alphabet_size,
                &[
                    (usize::from(symbols[0]), 2),
                    (usize::from(symbols[1]), 2),
                    (usize::from(symbols[2]), 2),
                    (usize::from(symbols[3]), 2),
                ],
            )),
            _ => panic!("test helper supports at most four channel symbols"),
        }
    }

    /// Writes a small non-level-zero VP8L entropy image with literal pixels.
    fn write_entropy_image_pixels(writer: &mut BitWriter, pixels: &[[u8; 4]]) {
        write_entropy_image_pixels_at_level(writer, pixels, false);
    }

    fn write_entropy_image_pixels_at_level(
        writer: &mut BitWriter,
        pixels: &[[u8; 4]],
        is_level0: bool,
    ) {
        assert!(!pixels.is_empty());
        writer.write_bits(0, 1).unwrap(); // color_cache_present
        if is_level0 {
            writer.write_bits(0, 1).unwrap(); // use_meta_huffman
        }
        let green = write_channel_code(
            writer,
            GREEN_ALPHABET_SIZE,
            &pixels.iter().map(|pixel| pixel[1]).collect::<Vec<_>>(),
        );
        let red = write_channel_code(
            writer,
            CHANNEL_ALPHABET_SIZE,
            &pixels.iter().map(|pixel| pixel[0]).collect::<Vec<_>>(),
        );
        let blue = write_channel_code(
            writer,
            CHANNEL_ALPHABET_SIZE,
            &pixels.iter().map(|pixel| pixel[2]).collect::<Vec<_>>(),
        );
        let alpha = write_channel_code(
            writer,
            CHANNEL_ALPHABET_SIZE,
            &pixels.iter().map(|pixel| pixel[3]).collect::<Vec<_>>(),
        );
        write_simple_code(writer, 0); // distance prefix

        for pixel in pixels {
            if let Some(lengths) = &green {
                write_symbol(writer, lengths, usize::from(pixel[1]));
            }
            if let Some(lengths) = &red {
                write_symbol(writer, lengths, usize::from(pixel[0]));
            }
            if let Some(lengths) = &blue {
                write_symbol(writer, lengths, usize::from(pixel[2]));
            }
            if let Some(lengths) = &alpha {
                write_symbol(writer, lengths, usize::from(pixel[3]));
            }
        }
    }

    fn meta_huffman_literal_stream(
        width: u32,
        height: u32,
        prefix_bits_field: u8,
        group_map: &[u16],
        group_pixels: &[[u8; 4]],
    ) -> Vec<u8> {
        let prefix_bits = prefix_bits_field + 2;
        let (map_width, map_height) = prefix_image_dimensions(width, height, prefix_bits).unwrap();
        assert_eq!(
            group_map.len(),
            usize::try_from(map_width * map_height).unwrap()
        );
        let max_group = usize::from(*group_map.iter().max().unwrap());
        assert_eq!(group_pixels.len(), max_group + 1);

        let mut writer = BitWriter::new();
        write_header(&mut writer, width, height, true);
        writer.write_bits(0, 1).unwrap(); // transform-list terminator
        writer.write_bits(0, 1).unwrap(); // color_cache_present
        writer.write_bits(1, 1).unwrap(); // use_meta_huffman
        writer.write_bits(u32::from(prefix_bits_field), 3).unwrap();
        let entropy_pixels: Vec<[u8; 4]> = group_map
            .iter()
            .map(|&group| [(group >> 8) as u8, group as u8, 0, 0])
            .collect();
        // The entropy image is a non-level-zero image and therefore starts
        // directly with its color-cache declaration.
        write_entropy_image_pixels(&mut writer, &entropy_pixels);

        // One fixed literal per group keeps the main data bit-free. The
        // groups are nevertheless written for every id through max_group,
        // including sparse group one below.
        for &pixel in group_pixels {
            for symbol in [pixel[1], pixel[0], pixel[2], pixel[3], 0] {
                write_simple_code(&mut writer, symbol);
            }
        }
        writer.into_bytes()
    }

    fn color_indexing_stream(
        width: u32,
        height: u32,
        palette_deltas: &[[u8; 4]],
        indexed_pixels: &[[u8; 4]],
    ) -> Vec<u8> {
        assert!((1..=256).contains(&palette_deltas.len()));
        let width_bits = webp_vp8l::color_index_width_bits(palette_deltas.len() as u16);
        let packed_width = width.div_ceil(1_u32 << width_bits);
        assert_eq!(
            indexed_pixels.len(),
            usize::try_from(packed_width * height).unwrap()
        );

        let mut writer = BitWriter::new();
        write_header(&mut writer, width, height, true);
        writer.write_bits(1, 1).unwrap(); // transform_present
        writer.write_bits(3, 2).unwrap(); // color indexing
        writer
            .write_bits(u32::try_from(palette_deltas.len() - 1).unwrap(), 8)
            .unwrap();
        write_entropy_image_pixels(&mut writer, palette_deltas);
        writer.write_bits(0, 1).unwrap(); // transform-list terminator
        write_entropy_image_pixels_at_level(&mut writer, indexed_pixels, true);
        writer.into_bytes()
    }

    fn all_transform_kinds_stream() -> Vec<u8> {
        let mut writer = BitWriter::new();
        write_header(&mut writer, 2, 1, true);

        writer.write_bits(1, 1).unwrap(); // predictor transform
        writer.write_bits(0, 2).unwrap();
        writer.write_bits(0, 3).unwrap(); // four-pixel blocks
        write_entropy_image_pixels(&mut writer, &[[0, 1, 0, 255]]);

        writer.write_bits(1, 1).unwrap(); // color transform
        writer.write_bits(1, 2).unwrap();
        writer.write_bits(0, 3).unwrap(); // four-pixel blocks
        write_entropy_image_pixels(&mut writer, &[[0, 0, 32, 0]]);

        writer.write_bits(1, 1).unwrap(); // subtract-green transform
        writer.write_bits(2, 2).unwrap();

        writer.write_bits(1, 1).unwrap(); // color indexing transform
        writer.write_bits(3, 2).unwrap();
        writer.write_bits(0, 8).unwrap(); // one palette entry
        write_entropy_image_pixels(&mut writer, &[[0, 32, 0, 0]]);

        writer.write_bits(0, 1).unwrap(); // transform-list terminator

        // Two one-bit palette indices, both zero, packed in green's low bits.
        write_entropy_image_pixels_at_level(&mut writer, &[[0, 0, 0, 0]], true);
        writer.into_bytes()
    }

    fn color_transform_stream(
        width: u32,
        height: u32,
        block_size_field: u8,
        transform_pixels: &[[u8; 4]],
        main_pixel: [u8; 4],
        following_transforms: &[u8],
    ) -> Vec<u8> {
        let block_size = 1_u32 << (u32::from(block_size_field) + 2);
        let table_width = width.div_ceil(block_size);
        let table_height = height.div_ceil(block_size);
        assert_eq!(
            transform_pixels.len(),
            usize::try_from(table_width * table_height).unwrap()
        );

        let mut writer = BitWriter::new();
        write_header(&mut writer, width, height, main_pixel[3] != 255);
        writer.write_bits(1, 1).unwrap(); // transform_present
        writer.write_bits(1, 2).unwrap(); // color transform
        writer.write_bits(u32::from(block_size_field), 3).unwrap();
        write_entropy_image_pixels(&mut writer, transform_pixels);
        for &transform in following_transforms {
            writer.write_bits(1, 1).unwrap(); // transform_present
            writer.write_bits(u32::from(transform), 2).unwrap();
        }
        writer.write_bits(0, 1).unwrap(); // transform-list terminator
        write_flat_entropy_image(&mut writer, main_pixel, true);
        writer.into_bytes()
    }

    fn predictor_stream(mode: u8) -> Vec<u8> {
        let mut writer = BitWriter::new();
        write_header(&mut writer, 2, 2, false);
        writer.write_bits(1, 1).unwrap(); // transform_present
        writer.write_bits(0, 2).unwrap(); // predictor transform
        writer.write_bits(0, 3).unwrap(); // 2 + 0 => four-pixel blocks

        // The predictor subimage is 1 by 1. It is a non-level-zero entropy
        // image, so this starts directly with color_cache_present; there is no
        // transform-list terminator or meta-Huffman flag here. Mode one is
        // carried in the green byte.
        write_flat_entropy_image(&mut writer, [0, mode, 0, 255], false);
        writer.write_bits(0, 1).unwrap(); // main transform-list terminator

        // All four residual samples are 1,1,1,0. Boundary rules reconstruct
        // the first row/column, while the lower-right pixel proves mode one
        // (left) is selected from the predictor subimage.
        write_flat_entropy_image(&mut writer, [1, 1, 1, 0], true);
        writer.into_bytes()
    }

    fn predictor_then_color_stream() -> Vec<u8> {
        let mut writer = BitWriter::new();
        write_header(&mut writer, 2, 2, false);
        writer.write_bits(1, 1).unwrap(); // predictor transform present
        writer.write_bits(0, 2).unwrap(); // predictor transform
        writer.write_bits(0, 3).unwrap(); // four-pixel blocks
        write_flat_entropy_image(&mut writer, [0, 1, 0, 255], false);
        writer.write_bits(1, 1).unwrap(); // color transform present
        writer.write_bits(1, 2).unwrap(); // color transform
        writer.write_bits(0, 3).unwrap(); // four-pixel blocks
        write_entropy_image_pixels(&mut writer, &[[0, 0, 32, 0]]);
        writer.write_bits(0, 1).unwrap(); // transform-list terminator
        write_flat_entropy_image(&mut writer, [0, 32, 0, 0], true);
        writer.into_bytes()
    }

    fn repeated_lz77_stream(width: u32, height: u32) -> Vec<u8> {
        let mut writer = BitWriter::new();
        write_header(&mut writer, width, height, false);
        writer.write_bits(0, 3).unwrap(); // no deferred features

        // Green symbol 2 is a literal. Green symbol 258 is length prefix 2,
        // which expands to a three-pixel copy.
        write_two_symbol_normal_code(&mut writer, GREEN_ALPHABET_SIZE, 2, 258);
        for symbol in [0, 0, 0] {
            write_simple_code(&mut writer, symbol);
        }
        write_simple_code(&mut writer, 0); // distance prefix 0 => code 1
        writer.write_bits(0, 1).unwrap(); // green literal symbol 2
        writer.write_bits(1, 1).unwrap(); // green copy symbol 258
        writer.into_bytes()
    }

    fn cache_hit_stream(pixel: [u8; 4], cache_bits: u8) -> Vec<u8> {
        let mut writer = BitWriter::new();
        write_header(&mut writer, 2, 1, pixel[3] != 255);
        writer.write_bits(0, 1).unwrap(); // transform_present
        writer.write_bits(1, 1).unwrap(); // color_cache_present
        writer.write_bits(u32::from(cache_bits), 4).unwrap(); // cache_bits
        writer.write_bits(0, 1).unwrap(); // use_meta_huffman

        let color = pack_argb(pixel[0], pixel[1], pixel[2], pixel[3]);
        let cache_index = webp_vp8l_color_cache::ColorCache::new(cache_bits)
            .unwrap()
            .index_of(color);
        let green = write_normal_code(
            &mut writer,
            GREEN_ALPHABET_SIZE + (1_usize << cache_bits),
            &[
                (usize::from(pixel[1]), 1),
                (GREEN_ALPHABET_SIZE + cache_index, 1),
            ],
        );
        for symbol in [pixel[0], pixel[2], pixel[3], 0] {
            write_simple_code(&mut writer, symbol);
        }
        write_symbol(&mut writer, &green, usize::from(pixel[1]));
        write_symbol(&mut writer, &green, GREEN_ALPHABET_SIZE + cache_index);
        writer.into_bytes()
    }

    fn cache_lz77_update_stream() -> (Vec<u8>, [u8; 2]) {
        let mut writer = BitWriter::new();
        write_header(&mut writer, 4, 1, true);
        writer.write_bits(0, 1).unwrap(); // transform_present
        writer.write_bits(1, 1).unwrap(); // color_cache_present
        writer.write_bits(1, 4).unwrap(); // cache_bits
        writer.write_bits(0, 1).unwrap(); // use_meta_huffman

        let cache = webp_vp8l_color_cache::ColorCache::new(1).unwrap();
        let first = 0_u8;
        let second = (1_u8..=u8::MAX)
            .find(|&alpha| {
                cache.index_of(pack_argb(0, 1, 0, alpha))
                    == cache.index_of(pack_argb(0, 1, 0, first))
            })
            .expect("a two-entry cache must have colliding alpha values");
        let cache_index = cache.index_of(pack_argb(0, 1, 0, first));

        let green = write_normal_code(
            &mut writer,
            GREEN_ALPHABET_SIZE + 2,
            &[(1, 1), (256, 2), (GREEN_ALPHABET_SIZE + cache_index, 2)],
        );
        write_simple_code(&mut writer, 0); // red
        write_simple_code(&mut writer, 0); // blue
        let alpha = write_normal_code(
            &mut writer,
            CHANNEL_ALPHABET_SIZE,
            &[(usize::from(first), 1), (usize::from(second), 1)],
        );
        write_simple_code(&mut writer, 13); // distance prefix => code 122 with extra 25

        write_symbol(&mut writer, &green, 1);
        write_symbol(&mut writer, &alpha, usize::from(first));
        write_symbol(&mut writer, &green, 1);
        write_symbol(&mut writer, &alpha, usize::from(second));
        write_symbol(&mut writer, &green, 256); // length prefix zero => one pixel
        writer.write_bits(25, 5).unwrap(); // distance prefix 13 => distance code 122 => distance two
        write_symbol(&mut writer, &green, GREEN_ALPHABET_SIZE + cache_index);
        (writer.into_bytes(), [first, second])
    }

    #[test]
    fn decodes_a_handwritten_single_literal_pixel() {
        let data = literal_stream(1, 1, [0x12, 0x34, 0x56, 0x78]);
        let image = decode_literal_only(&data, &limits()).unwrap();
        assert_eq!((image.header.width, image.header.height), (1, 1));
        assert_eq!(image.rgba, [0x12, 0x34, 0x56, 0x78]);
    }

    #[test]
    fn decodes_multiple_pixels_from_zero_bit_single_symbol_tables() {
        let data = literal_stream(3, 2, [1, 2, 3, 255]);
        let image = decode_literal_only(&data, &limits()).unwrap();
        assert_eq!(image.rgba, [1, 2, 3, 255].repeat(6));
    }

    #[test]
    fn applies_subtract_green_to_handwritten_residual_pixels() {
        // Stored channels are residual red, green, residual blue, alpha.
        // Inversion adds green to red and blue modulo 256.
        let data = literal_stream_with_transforms(1, 1, [0xf0, 0x30, 0xee, 0x80], &[2]);
        let image = decode_literal_only(&data, &limits()).unwrap();
        assert_eq!(image.rgba, [0x20, 0x30, 0x1e, 0x80]);
    }

    #[test]
    fn decodes_color_transform_with_specified_argb_multiplier_mapping() {
        // The transform pixel is ARGB on wire. R feeds red-to-blue, G feeds
        // green-to-blue, B feeds green-to-red, and alpha is ignored.
        let data = color_transform_stream(
            1,
            1,
            0,
            &[[0x20, 0x80, 0x01, 0x55]],
            [3, 0x80, 0, 0x44],
            &[],
        );
        let image = decode_no_transform(&data, &limits()).unwrap();
        // Green is signed -128. Red becomes 3 + (1 * -128 >> 5) = 255;
        // blue then receives (-128 * -128 >> 5) + (32 * -1 >> 5).
        assert_eq!(image.rgba, [255, 128, 255, 0x44]);
    }

    #[test]
    fn color_transform_selects_multipliers_at_partial_block_boundaries() {
        // 5x5 pixels with four-pixel blocks yield a 2x2 transform image.
        // Each block carries a different green-to-red multiplier in B.
        let data = color_transform_stream(
            5,
            5,
            0,
            &[[0, 0, 0, 0], [0, 0, 1, 0], [0, 0, 2, 0], [0, 0, 0xff, 0]],
            [0, 32, 0, 1],
            &[],
        );
        let image = decode_no_transform(&data, &limits()).unwrap();
        let rgba_at = |x: usize, y: usize| &image.rgba[(y * 5 + x) * 4..(y * 5 + x + 1) * 4];
        assert_eq!(rgba_at(0, 0), [0, 32, 0, 1]);
        assert_eq!(rgba_at(4, 0), [1, 32, 0, 1]);
        assert_eq!(rgba_at(0, 4), [2, 32, 0, 1]);
        assert_eq!(rgba_at(4, 4), [255, 32, 0, 1]);
    }

    #[test]
    fn inverse_transforms_run_in_reverse_wire_order_with_subtract_green() {
        // Color appears before subtract-green on wire, so subtract-green is
        // inverted first. Its reconstructed green then drives color's B=32
        // green-to-red multiplier.
        let data = color_transform_stream(1, 1, 0, &[[0, 0, 32, 0]], [0, 32, 0, 9], &[2]);
        let image = decode_no_transform(&data, &limits()).unwrap();
        assert_eq!(image.rgba, [64, 32, 32, 9]);
    }

    #[test]
    fn inverse_transforms_run_in_reverse_wire_order_with_predictor() {
        // Predictor appears before color on wire, so color is inverted first.
        // The predictor then reconstructs each color-corrected residual.
        let image = decode_no_transform(&predictor_then_color_stream(), &limits()).unwrap();
        assert_eq!(
            image.rgba,
            [
                32, 32, 0, 255, // top-left boundary predictor
                64, 64, 0, 255, // top row uses left
                64, 64, 0, 255, // left column uses top
                96, 96, 0, 255, // mode one uses left
            ]
        );
    }

    #[test]
    fn rgba_predictor_rows_match_the_scalar_reference_for_every_mode() {
        let descriptor = BlockTransformDescriptor {
            image_width: 3,
            image_height: 2,
            block_size_bits: 2,
            transform_width: 1,
            transform_height: 1,
        };
        let residuals = vec![
            0x1020_3040,
            0x5060_7080,
            0x90a0_b0c0,
            0xd0e0_f001,
            0x1234_5678,
            0x9abc_def0,
        ];

        for mode_value in 0_u8..=13 {
            let mode = PredictorMode::try_from(mode_value).unwrap();
            let mut expected = residuals.clone();
            for y in 0..2 {
                for x in 0..3 {
                    let offset = y * 3 + x;
                    let residual = argb_to_rgba(expected[offset]);
                    let prediction = if x == 0 && y == 0 {
                        Rgba::OPAQUE_BLACK
                    } else if y == 0 {
                        argb_to_rgba(expected[offset - 1])
                    } else if x == 0 {
                        argb_to_rgba(expected[offset - 3])
                    } else {
                        let left = argb_to_rgba(expected[offset - 1]);
                        let top = argb_to_rgba(expected[offset - 3]);
                        let top_left = argb_to_rgba(expected[offset - 4]);
                        let top_right = if x == 2 {
                            argb_to_rgba(expected[y * 3])
                        } else {
                            argb_to_rgba(expected[offset - 2])
                        };
                        webp_vp8l_transform::predict(mode, left, top, top_left, top_right)
                    };
                    expected[offset] = pack_argb(
                        residual.red.wrapping_add(prediction.red),
                        residual.green.wrapping_add(prediction.green),
                        residual.blue.wrapping_add(prediction.blue),
                        residual.alpha.wrapping_add(prediction.alpha),
                    );
                }
            }

            let mut actual = residuals.clone();
            inverse_predictor_argb_reference(
                &mut actual,
                descriptor,
                &[u32::from(mode_value) << 8],
            )
            .unwrap();
            assert_eq!(actual, expected, "predictor mode {mode_value}");

            let mut actual_rgba = Vec::with_capacity(residuals.len() * 4);
            for &pixel in &residuals {
                actual_rgba.extend_from_slice(&unpack_rgba(pixel));
            }
            inverse_predictor_rgba(&mut actual_rgba, descriptor, &[u32::from(mode_value) << 8])
                .unwrap();
            let actual_rgba = actual_rgba
                .chunks_exact(4)
                .map(|pixel| pack_argb(pixel[0], pixel[1], pixel[2], pixel[3]))
                .collect::<Vec<_>>();
            assert_eq!(actual_rgba, expected, "RGBA predictor mode {mode_value}");

            let fused = inverse_predictor_argb_to_rgba(
                &residuals,
                descriptor,
                &[u32::from(mode_value) << 8],
            )
            .unwrap();
            let fused = fused
                .chunks_exact(4)
                .map(|pixel| pack_argb(pixel[0], pixel[1], pixel[2], pixel[3]))
                .collect::<Vec<_>>();
            assert_eq!(fused, expected, "fused predictor mode {mode_value}");
        }
    }

    #[test]
    fn color_transform_storage_counts_toward_the_decoder_limit() {
        let data = color_transform_stream(
            5,
            5,
            0,
            &[[0, 0, 0, 0], [0, 0, 1, 0], [0, 0, 2, 0], [0, 0, 3, 0]],
            [0, 32, 0, 1],
            &[],
        );
        // Four compact multiplier entries (12 B), 25 packed main pixels
        // (100 B), and final RGBA (100 B) coexist during main entropy decode.
        let limited = DecodeLimits {
            max_alloc_bytes: 211,
            ..limits()
        };
        assert_eq!(
            decode_no_transform(&data, &limited).unwrap_err().kind(),
            DecodeErrorKind::LimitExceeded
        );
    }

    #[test]
    fn decodes_predictor_subimage_without_a_nested_transform_list() {
        let image = decode_no_transform(&predictor_stream(1), &limits()).unwrap();
        assert_eq!(
            image.rgba,
            [
                1, 1, 1, 255, // top-left: opaque black + residual
                2, 2, 2, 255, // top row: left + residual
                2, 2, 2, 255, // left column: top + residual
                3, 3, 3, 255, // mode one: left + residual
            ]
        );
    }

    #[test]
    fn rejects_predictor_modes_outside_the_wire_range() {
        assert_eq!(
            decode_no_transform(&predictor_stream(14), &limits())
                .unwrap_err()
                .kind(),
            DecodeErrorKind::InvalidBitstream
        );
    }

    #[test]
    fn predictor_subimage_storage_counts_toward_the_allocation_limit() {
        // One packed predictor-mode pixel (4 B), four packed main pixels
        // (16 B), and final RGBA (16 B) are conservatively accounted while
        // main entropy is decoded.
        let limited = DecodeLimits {
            max_alloc_bytes: 35,
            ..limits()
        };
        assert_eq!(
            decode_no_transform(&predictor_stream(1), &limited)
                .unwrap_err()
                .kind(),
            DecodeErrorKind::LimitExceeded
        );
    }

    #[test]
    fn inverse_subtract_green_preserves_green_and_alpha_for_each_pixel() {
        let mut pixels = [
            pack_argb(0xf0, 0x30, 0xee, 0x80),
            pack_argb(0x01, 0xff, 0x02, 0x7f),
        ];
        inverse_subtract_green_argb(&mut pixels);
        assert_eq!(unpack_rgba(pixels[0]), [0x20, 0x30, 0x1e, 0x80]);
        assert_eq!(unpack_rgba(pixels[1]), [0x00, 0xff, 0x01, 0x7f]);
    }

    #[test]
    fn decodes_meta_huffman_groups_with_round_up_and_sparse_ids() {
        // prefix_bits = 2 produces a 3x2 entropy image for this 9x5 output.
        // Group one appears only in the last column, while group two appears
        // in other blocks, so decoding must parse and retain all three groups
        // and must select them from the red/green 16-bit meta code.
        let group_map = [0_u16, 2, 1, 1, 0, 2];
        let group_pixels = [[1, 10, 100, 255], [2, 20, 110, 254], [3, 30, 120, 253]];
        let image = decode_no_transform(
            &meta_huffman_literal_stream(9, 5, 0, &group_map, &group_pixels),
            &limits(),
        )
        .unwrap();

        let mut expected = Vec::new();
        for y in 0..5_usize {
            for x in 0..9_usize {
                let group = group_map[(y / 4) * 3 + x / 4];
                expected.extend_from_slice(&group_pixels[usize::from(group)]);
            }
        }
        assert_eq!(image.rgba, expected);
    }

    #[test]
    fn meta_prefix_dimensions_round_up_for_every_prefix_bits_value() {
        for field in 0..=7_u8 {
            let bits = field + 2;
            let (width, height) = prefix_image_dimensions(513, 1025, bits).unwrap();
            let block = 1_u32 << bits;
            assert_eq!(width, 513_u32.div_ceil(block));
            assert_eq!(height, 1025_u32.div_ceil(block));
        }
    }

    #[test]
    fn meta_huffman_group_id_uses_both_red_and_green_bytes() {
        // 0x0100 must select group 256, not group zero. The 256 preceding
        // groups are still present in the bitstream and must be parsed before
        // the selected group.
        let mut group_pixels = vec![[0, 0, 0, 0]; 257];
        group_pixels[256] = [9, 8, 7, 6];
        let image = decode_no_transform(
            &meta_huffman_literal_stream(1, 1, 0, &[0x0100], &group_pixels),
            &limits(),
        )
        .unwrap();
        assert_eq!(image.rgba, [9, 8, 7, 6]);
    }

    #[test]
    fn meta_huffman_tables_and_maps_count_toward_allocation_limit() {
        let data = meta_huffman_literal_stream(1, 1, 0, &[0], &[[1, 2, 3, 4]]);
        let limited = DecodeLimits {
            // The nested entropy image itself is tiny; this limit is crossed
            // by the conservative retained/transient prefix-table accounting.
            max_alloc_bytes: 16 * 1024,
            ..limits()
        };
        assert_eq!(
            decode_no_transform(&data, &limited).unwrap_err().kind(),
            DecodeErrorKind::LimitExceeded
        );
    }

    #[test]
    fn truncated_color_indexing_palette_reports_eof() {
        let mut writer = BitWriter::new();
        write_header(&mut writer, 1, 1, false);
        writer.write_bits(1, 1).unwrap(); // transform_present
        writer.write_bits(3, 2).unwrap(); // color indexing
        writer.write_bits(0, 8).unwrap(); // one palette entry
        assert_eq!(
            decode_literal_only(&writer.into_bytes(), &limits())
                .unwrap_err()
                .kind(),
            DecodeErrorKind::UnexpectedEof
        );
    }

    #[test]
    fn decodes_packed_color_indices_and_palette_deltas() {
        // A four-entry palette packs four two-bit indices in each source
        // green byte.
        let data = color_indexing_stream(
            4,
            1,
            &[[10, 20, 30, 40], [5, 5, 5, 5], [7, 7, 7, 7], [9, 9, 9, 9]],
            &[[0xa5, 0b0100_0100, 0x5a, 0x33]],
        );
        let image = decode_no_transform(&data, &limits()).unwrap();
        let first = [10, 20, 30, 40];
        let second = [15, 25, 35, 45];
        assert_eq!(image.rgba, [first, second, first, second].concat());
    }

    #[test]
    fn color_indexing_handles_each_palette_packing_boundary() {
        for (size, width) in [
            (2, 9_u32),
            (3, 5),
            (4, 5),
            (5, 3),
            (16, 3),
            (17, 3),
            (256, 3),
        ] {
            let palette = vec![[7, 8, 9, 10]; size];
            let width_bits = webp_vp8l::color_index_width_bits(size as u16);
            let packed_width = width.div_ceil(1_u32 << width_bits);
            let indexed = vec![[0, 0, 0, 0]; usize::try_from(packed_width).unwrap()];
            let image = decode_no_transform(
                &color_indexing_stream(width, 1, &palette, &indexed),
                &limits(),
            )
            .unwrap();
            assert_eq!(
                image.rgba,
                [7, 8, 9, 10].repeat(width as usize),
                "size {size}"
            );
        }
    }

    #[test]
    fn invalid_packed_palette_indices_become_transparent_black() {
        // Palette size three selects two-bit indices. The first index is
        // three (invalid), while every remaining index is zero.
        let image = decode_no_transform(
            &color_indexing_stream(
                4,
                1,
                &[[1, 1, 1, 1], [1, 1, 1, 1], [1, 1, 1, 1]],
                &[[0xaa, 0b0000_0011, 0x55, 0x99]],
            ),
            &limits(),
        )
        .unwrap();
        assert_eq!(
            image.rgba,
            [
                0, 0, 0, 0, // invalid palette index
                1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
            ]
        );
    }

    #[test]
    fn color_indexing_expands_before_other_inverse_transforms() {
        let image = decode_no_transform(&all_transform_kinds_stream(), &limits()).unwrap();
        // Wire order is predictor, color, subtract-green, indexing. Reverse
        // order expands palette indices first, then applies the other three
        // transforms at their original two-pixel width.
        assert_eq!(
            image.rgba,
            [
                64, 32, 32, 255, // opaque-black predictor boundary
                128, 64, 64, 255, // top-row left predictor
            ]
        );
    }

    #[test]
    fn color_indexing_expansion_counts_packed_palette_and_output_buffers() {
        let data = color_indexing_stream(2, 1, &[[1, 2, 3, 4], [1, 2, 3, 4]], &[[0, 0, 0, 0]]);
        // The retained palette (8 B), narrow packed output (4 B), expanded
        // output (8 B), and final RGBA (8 B) coexist during expansion.
        let limited = DecodeLimits {
            max_alloc_bytes: 27,
            ..limits()
        };
        assert_eq!(
            decode_no_transform(&data, &limited).unwrap_err().kind(),
            DecodeErrorKind::LimitExceeded
        );
    }

    #[test]
    fn truncated_predictor_subimage_reports_eof_after_prior_transforms() {
        let mut writer = BitWriter::new();
        write_header(&mut writer, 1, 1, false);
        writer.write_bits(1, 1).unwrap(); // subtract-green present
        writer.write_bits(2, 2).unwrap(); // subtract-green
        writer.write_bits(1, 1).unwrap(); // predictor present
        writer.write_bits(0, 2).unwrap(); // predictor
        writer.write_bits(0, 3).unwrap(); // predictor block_size_bits
        assert_eq!(
            decode_literal_only(&writer.into_bytes(), &limits())
                .unwrap_err()
                .kind(),
            DecodeErrorKind::UnexpectedEof
        );
    }

    #[test]
    fn decodes_overlapping_lz77_copy_with_distance_one() {
        let data = repeated_lz77_stream(1, 4);
        let image = decode_literal_only(&data, &limits()).unwrap();
        assert_eq!(image.rgba, [0, 2, 0, 0].repeat(4));
    }

    #[test]
    fn decodes_color_cache_hit_after_a_literal() {
        let pixel = [0x12, 0x34, 0x56, 0x78];
        let image = decode_no_transform(&cache_hit_stream(pixel, 1), &limits()).unwrap();
        assert_eq!(image.rgba, pixel.repeat(2));
    }

    #[test]
    fn accepts_the_largest_color_cache_exponent() {
        let pixel = [0x12, 0x34, 0x56, 0x78];
        let image = decode_no_transform(&cache_hit_stream(pixel, 11), &limits()).unwrap();
        assert_eq!(image.rgba, pixel.repeat(2));
    }

    #[test]
    fn cache_allocation_counts_toward_the_decoder_limit() {
        let pixel = [0x12, 0x34, 0x56, 0x78];
        let data = cache_hit_stream(pixel, 11);
        // Two packed pixels (8 B), 2048 cache entries (8192 B), and the two
        // RGBA pixels (8 B) coexist while final output is allocated.
        let limited = DecodeLimits {
            max_alloc_bytes: 8207,
            ..limits()
        };
        assert_eq!(
            decode_no_transform(&data, &limited).unwrap_err().kind(),
            DecodeErrorKind::LimitExceeded
        );
    }

    #[test]
    fn lz77_pixels_update_color_cache_before_the_next_symbol() {
        let (data, alpha) = cache_lz77_update_stream();
        let image = decode_no_transform(&data, &limits()).unwrap();
        let first = [0, 1, 0, alpha[0]];
        let second = [0, 1, 0, alpha[1]];
        // The cache hit must resolve to `first`: the LZ77 reference copied it
        // after `second` had overwritten their shared cache slot.
        assert_eq!(image.rgba, [first, second, first, first].concat());
    }

    #[test]
    fn rejects_invalid_or_truncated_color_cache_headers_without_panicking() {
        let mut invalid = cache_hit_stream([1, 2, 3, 4], 1);
        let cache_bits_position = HEADER_LEN * 8 + 2;
        for offset in 0..4 {
            let position = cache_bits_position + offset;
            invalid[position / 8] &= !(1 << (position % 8));
        }
        assert_eq!(
            decode_no_transform(&invalid, &limits()).unwrap_err().kind(),
            DecodeErrorKind::InvalidBitstream
        );

        let data = cache_hit_stream([1, 2, 3, 4], 1);
        for length in 0..data.len() {
            let result = decode_no_transform(&data[..length], &limits());
            assert!(
                result.is_err(),
                "truncation length {length} unexpectedly decoded"
            );
        }
    }

    #[test]
    fn rejects_lz77_distance_before_produced_pixels() {
        // Distance code one means one scanline. At width two, it points back
        // two pixels although only the initial literal has been produced.
        let data = repeated_lz77_stream(2, 2);
        assert_eq!(
            decode_no_transform(&data, &limits()).unwrap_err().kind(),
            DecodeErrorKind::InvalidBitstream
        );
    }

    #[test]
    fn rejects_lz77_copy_that_exceeds_image_output() {
        let data = repeated_lz77_stream(1, 3);
        assert_eq!(
            decode_no_transform(&data, &limits()).unwrap_err().kind(),
            DecodeErrorKind::InvalidBitstream
        );
    }

    #[test]
    fn input_and_allocation_limits_apply_before_output_allocation() {
        let data = literal_stream(2, 2, [1, 2, 3, 4]);
        let input_limited = DecodeLimits {
            max_input_bytes: data.len() - 1,
            ..limits()
        };
        assert_eq!(
            decode_literal_only(&data, &input_limited)
                .unwrap_err()
                .kind(),
            DecodeErrorKind::LimitExceeded
        );
        let allocation_limited = DecodeLimits {
            max_alloc_bytes: 15,
            ..limits()
        };
        assert_eq!(
            decode_literal_only(&data, &allocation_limited)
                .unwrap_err()
                .kind(),
            DecodeErrorKind::LimitExceeded
        );
    }

    #[test]
    fn work_budget_covers_headers_tables_and_literal_symbols() {
        let data = literal_stream(1, 1, [1, 2, 3, 4]);
        let limited = DecodeLimits {
            max_work_units: 12, // 3 stream flags + 5 tables + 4 channel symbols
            ..limits()
        };
        assert!(decode_literal_only(&data, &limited).is_ok());
        let exhausted = DecodeLimits {
            max_work_units: 11,
            ..limits()
        };
        assert_eq!(
            decode_literal_only(&data, &exhausted).unwrap_err().kind(),
            DecodeErrorKind::LimitExceeded
        );
    }

    #[test]
    fn work_budget_covers_lz77_symbol_expansion_and_copy() {
        let data = repeated_lz77_stream(1, 4);
        let limited = DecodeLimits {
            // 3 flags + 5 tables + 4 literal symbols + 1 copy symbol +
            // length expansion + distance symbol + distance expansion + copy.
            max_work_units: 19,
            ..limits()
        };
        assert!(decode_no_transform(&data, &limited).is_ok());
        let exhausted = DecodeLimits {
            max_work_units: 18,
            ..limits()
        };
        assert_eq!(
            decode_no_transform(&data, &exhausted).unwrap_err().kind(),
            DecodeErrorKind::LimitExceeded
        );
    }

    #[test]
    fn truncation_never_panics_and_reports_eof() {
        let data = literal_stream(1, 1, [1, 2, 3, 4]);
        for length in 0..data.len() {
            let error = decode_literal_only(&data[..length], &limits()).unwrap_err();
            assert_eq!(
                error.kind(),
                DecodeErrorKind::UnexpectedEof,
                "length {length}"
            );
        }

        let transformed = literal_stream_with_transforms(1, 1, [1, 2, 3, 4], &[2]);
        for length in 0..transformed.len() {
            let error = decode_literal_only(&transformed[..length], &limits()).unwrap_err();
            assert_eq!(
                error.kind(),
                DecodeErrorKind::UnexpectedEof,
                "subtract-green truncation length {length}"
            );
        }

        let color_transformed = color_transform_stream(1, 1, 0, &[[0, 0, 1, 0]], [1, 2, 3, 4], &[]);
        for length in 0..color_transformed.len() {
            let error = decode_no_transform(&color_transformed[..length], &limits()).unwrap_err();
            assert_eq!(
                error.kind(),
                DecodeErrorKind::UnexpectedEof,
                "color-transform truncation length {length}"
            );
        }
    }
}
