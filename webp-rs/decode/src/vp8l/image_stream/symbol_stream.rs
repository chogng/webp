use crate::BitReader;
use crate::DecodeError;
use crate::DecodeErrorKind;
use crate::DecodeLimits;
use crate::WorkBudget;
use crate::vp8l::allocation::check_allocation_budget;
use crate::vp8l::allocation::checked_transform_bytes;
use crate::vp8l::allocation::color_cache_size;
use crate::vp8l::allocation::pixel_count;
use crate::vp8l::backward_references::decode_distance_shifted;
use crate::vp8l::backward_references::decode_length_shifted;
use crate::vp8l::backward_references::distance_code_to_distance;
use crate::vp8l::huffman::FastHuffmanTable;
use crate::vp8l::huffman::MAX_SECONDARY_TABLE_STORAGE_BYTES;
use crate::vp8l::huffman::ROOT_TABLE_STORAGE_BYTES;
#[cfg(test)]
use crate::vp8l::image_stream::decode_profile::record_entropy_path;
use crate::vp8l::image_stream::huffman_groups::CHANNEL_ALPHABET_SIZE;
use crate::vp8l::image_stream::huffman_groups::DISTANCE_ALPHABET_SIZE;
use crate::vp8l::image_stream::huffman_groups::GREEN_ALPHABET_SIZE;
use crate::vp8l::image_stream::huffman_groups::GreenOrLiteral;
use crate::vp8l::image_stream::huffman_groups::HUFFMAN_TABLES_PER_GROUP;
use crate::vp8l::image_stream::huffman_groups::HuffmanCodes;
use crate::vp8l::image_stream::huffman_groups::MAX_HUFFMAN_CODE_STORAGE_BYTES;
use crate::vp8l::image_stream::huffman_groups::decode_fast_symbol;
use crate::vp8l::image_stream::huffman_groups::decode_green_or_literal;
use crate::vp8l::image_stream::huffman_groups::read_huffman_codes;
use crate::vp8l::image_stream::pixel_sink::PixelOutput;
use crate::vp8l::pixel::pack_argb;

/// Decodes VP8L's entropy image syntax at either nesting level.
///
/// A main-level image may carry a spatial meta-Huffman image. Predictor and
/// transform subimages are `is_level0 = false`, so their Huffman stream begins
/// directly after the color-cache declaration and cannot recursively carry
/// meta-Huffman data.
#[allow(clippy::too_many_arguments)]
pub(in crate::vp8l) fn decode_image_data(
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
    shifted_bits: &mut crate::ShiftedBitReader<'_, '_>,
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
                let group_index =
                    usize::from(*codes.group_map.get(map_index).ok_or_else(|| {
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
    let entropy_image = decode_image_data(
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

pub(in crate::vp8l) fn prefix_image_dimensions(
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

#[cfg(test)]
#[path = "symbol_stream_tests.rs"]
mod tests;
