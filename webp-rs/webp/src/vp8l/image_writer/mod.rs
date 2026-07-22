//! Minimal static VP8L lossless encoding.
//!
//! This M6 slice writes a single entropy group with reversible color,
//! subtract-green, and predictor transforms, a bounded color cache, and
//! distance-one backward references. Entropy tables use deterministic
//! frequency-ranked balanced Huffman codes. Small palette images use color
//! indexing.

use crate::BitWriter;
use crate::EncodeError;
use crate::vp8l::backward_references::prefix::encode_prefix as vp8l_prefix;
use crate::vp8l::color_cache::hash_color;
use crate::vp8l::header::MAX_DIMENSION;
use crate::vp8l::header::SIGNATURE;
use crate::vp8l::huffman::symbol_writer::EncodingTable;
use crate::vp8l::huffman::symbol_writer::canonical_table;
use crate::vp8l::huffman::symbol_writer::write_canonical_symbol;
use crate::vp8l::huffman::symbol_writer::write_simple_table;
use crate::vp8l::huffman::symbol_writer::write_table_symbol;

const GREEN_ALPHABET_SIZE: usize = 256 + 24;
const CHANNEL_ALPHABET_SIZE: usize = 256;
const DISTANCE_ALPHABET_SIZE: usize = 40;
const PREDICTOR_BLOCK_SIZE: u32 = 4;
const LEFT_PREDICTOR_MODE: u8 = 1;
const MAX_ENCODER_COLOR_CACHE_BITS: u8 = 4;
const MAX_COLOR_CACHE_SIZE: usize = 1 << MAX_ENCODER_COLOR_CACHE_BITS;
const FIRST_CACHE_SYMBOL: usize = GREEN_ALPHABET_SIZE;
const MAIN_GREEN_ALPHABET_SIZE: usize = GREEN_ALPHABET_SIZE + MAX_COLOR_CACHE_SIZE;
const CODE_LENGTH_CODE_ORDER: [usize; 19] = [
    17, 18, 0, 1, 2, 3, 4, 5, 16, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15,
];
const MAX_ENCODER_PALETTE_SIZE: usize = 16;
pub(crate) const COLOR_TRANSFORM_BLOCK_BITS: u8 = 7;
const MIN_COLOR_TRANSFORM_PIXELS: usize = 256;

#[derive(Clone, Copy)]
pub(crate) enum EntropyToken {
    Literal([u8; 4]),
    Cache(usize),
    Copy { length: usize },
}

pub(crate) struct EntropyFrequencies {
    green: [u32; MAIN_GREEN_ALPHABET_SIZE],
    green_len: usize,
    red: [u32; CHANNEL_ALPHABET_SIZE],
    blue: [u32; CHANNEL_ALPHABET_SIZE],
    alpha: [u32; CHANNEL_ALPHABET_SIZE],
    distance: [u32; DISTANCE_ALPHABET_SIZE],
}

pub(crate) struct EntropyTables {
    green: EncodingTable,
    red: EncodingTable,
    blue: EncodingTable,
    alpha: EncodingTable,
    distance: EncodingTable,
}

pub(crate) struct PalettePlan {
    entries: Vec<[u8; 4]>,
    indexed_rgba: Vec<u8>,
    indexed_width: usize,
}

#[derive(Clone, Copy)]
pub(crate) struct ColorTransformPlan {
    pub(crate) green_to_red: i8,
    pub(crate) green_to_blue: i8,
    pub(crate) red_to_blue: i8,
}

#[path = "single_plan.rs"]
mod single_plan;
#[path = "spatial_cluster.rs"]
mod spatial_cluster;
#[path = "spatial_packet_writer.rs"]
mod spatial_packet_writer;
#[path = "spatial_plan.rs"]
pub(crate) mod spatial_plan;
#[path = "spatial_writer.rs"]
pub(crate) mod spatial_writer;

#[cfg(test)]
#[path = "coarse_spatial_tests.rs"]
mod coarse_spatial_tests;
#[cfg(test)]
#[path = "product_benchmark_tests.rs"]
mod product_benchmark_tests;

pub(crate) fn encode_vp8l_payload(
    width: u32,
    height: u32,
    rgba: &[u8],
) -> Result<(Vec<u8>, bool), EncodeError> {
    validate_input(width, height, rgba)?;
    let has_alpha = rgba.chunks_exact(4).any(|pixel| pixel[3] != u8::MAX);
    let width_usize = usize::try_from(width).map_err(|_| EncodeError::input_size_overflow())?;
    if let Some(palette) = try_make_palette_plan(rgba, width_usize)? {
        return encode_palette_vp8l_payload(width, height, has_alpha, palette);
    }

    let color_transform = select_color_transform(rgba);
    let transformed = match color_transform {
        Some(plan) => apply_forward_color_transform(rgba, plan)?,
        None => rgba.to_vec(),
    };

    let mut bits = BitWriter::new();
    write_vp8l_header(&mut bits, width, height, has_alpha)?;

    if let Some(plan) = color_transform {
        write_bits(&mut bits, 1, 1)?; // Color transform follows.
        write_bits(&mut bits, 1, 2)?; // VP8L color transform type.
        write_bits(&mut bits, u32::from(COLOR_TRANSFORM_BLOCK_BITS - 2), 3)?;
        write_color_transform_image(&mut bits, width, height, plan)?;
    }
    write_bits(&mut bits, 1, 1)?; // Subtract-green transform follows.
    write_bits(&mut bits, 2, 2)?; // VP8L subtract-green transform type.
    let use_left_predictor = select_left_predictor(&transformed, width_usize);
    if use_left_predictor {
        write_bits(&mut bits, 1, 1)?; // Predictor transform follows.
        write_bits(&mut bits, 0, 2)?; // VP8L predictor transform type.
        write_bits(&mut bits, 0, 3)?; // 2 + 0 => four-pixel predictor blocks.
        write_predictor_mode_image(&mut bits, width, height)?;
    }
    write_bits(&mut bits, 0, 1)?; // Transform-list terminator.

    let color_cache_bits =
        select_color_cache_bits(&transformed, width_usize, true, use_left_predictor);
    let (tokens, frequencies) = collect_entropy_tokens(
        &transformed,
        width_usize,
        true,
        use_left_predictor,
        color_cache_bits,
    )?;
    let tables = write_main_entropy_image_prefix(&mut bits, &frequencies, color_cache_bits)?;
    for token in tokens {
        match token {
            EntropyToken::Cache(index) => {
                write_table_symbol(&mut bits, &tables.green, FIRST_CACHE_SYMBOL + index)?;
            }
            EntropyToken::Literal(residual) => {
                // VP8L literal syntax orders channels green, red, blue, alpha.
                write_table_symbol(&mut bits, &tables.green, usize::from(residual[1]))?;
                write_table_symbol(&mut bits, &tables.red, usize::from(residual[0]))?;
                write_table_symbol(&mut bits, &tables.blue, usize::from(residual[2]))?;
                write_table_symbol(&mut bits, &tables.alpha, usize::from(residual[3]))?;
            }
            EntropyToken::Copy { length } => write_lz77_copy(&mut bits, &tables, length)?,
        }
    }

    Ok((bits.into_bytes(), has_alpha))
}

fn encode_palette_vp8l_payload(
    width: u32,
    height: u32,
    has_alpha: bool,
    palette: PalettePlan,
) -> Result<(Vec<u8>, bool), EncodeError> {
    let mut bits = BitWriter::new();
    write_vp8l_header(&mut bits, width, height, has_alpha)?;
    write_bits(&mut bits, 1, 1)?; // Color-indexing transform follows.
    write_bits(&mut bits, 3, 2)?;
    write_bits(
        &mut bits,
        u32::try_from(palette.entries.len() - 1)
            .map_err(|_| EncodeError::output_size_overflow())?,
        8,
    )?;
    write_palette_image(&mut bits, &palette.entries)?;
    write_bits(&mut bits, 0, 1)?; // Transform-list terminator.

    let color_cache_bits =
        select_color_cache_bits(&palette.indexed_rgba, palette.indexed_width, false, false);
    let (tokens, frequencies) = collect_entropy_tokens(
        &palette.indexed_rgba,
        palette.indexed_width,
        false,
        false,
        color_cache_bits,
    )?;
    let tables = write_main_entropy_image_prefix(&mut bits, &frequencies, color_cache_bits)?;
    for token in tokens {
        match token {
            EntropyToken::Cache(index) => {
                write_table_symbol(&mut bits, &tables.green, FIRST_CACHE_SYMBOL + index)?;
            }
            EntropyToken::Literal(pixel) => {
                write_table_symbol(&mut bits, &tables.green, usize::from(pixel[1]))?;
                write_table_symbol(&mut bits, &tables.red, usize::from(pixel[0]))?;
                write_table_symbol(&mut bits, &tables.blue, usize::from(pixel[2]))?;
                write_table_symbol(&mut bits, &tables.alpha, usize::from(pixel[3]))?;
            }
            EntropyToken::Copy { length } => write_lz77_copy(&mut bits, &tables, length)?,
        }
    }
    Ok((bits.into_bytes(), has_alpha))
}

fn write_vp8l_header(
    writer: &mut BitWriter,
    width: u32,
    height: u32,
    has_alpha: bool,
) -> Result<(), EncodeError> {
    write_bits(writer, u32::from(SIGNATURE), 8)?;
    write_bits(writer, width - 1, 14)?;
    write_bits(writer, height - 1, 14)?;
    write_bits(writer, u32::from(has_alpha), 1)?;
    write_bits(writer, 0, 3) // VP8L version.
}

pub(crate) fn try_make_palette_plan(
    rgba: &[u8],
    width: usize,
) -> Result<Option<PalettePlan>, EncodeError> {
    let mut entries = Vec::new();
    entries
        .try_reserve_exact(MAX_ENCODER_PALETTE_SIZE)
        .map_err(|_| EncodeError::allocation_failed())?;
    let mut indices = Vec::new();
    indices
        .try_reserve_exact(rgba.len() / 4)
        .map_err(|_| EncodeError::allocation_failed())?;
    for pixel in rgba.chunks_exact(4) {
        let pixel = [pixel[0], pixel[1], pixel[2], pixel[3]];
        let index = match entries.iter().position(|entry| *entry == pixel) {
            Some(index) => index,
            None if entries.len() < MAX_ENCODER_PALETTE_SIZE => {
                entries.push(pixel);
                entries.len() - 1
            }
            None => return Ok(None),
        };
        indices.push(u8::try_from(index).expect("bounded palette index fits u8"));
    }
    // A literal one-pixel image is smaller and clearer than its palette
    // descriptor plus nested palette image; otherwise select every bounded
    // palette deterministically.
    if indices.len() < 2 {
        return Ok(None);
    }
    let indices_per_pixel = match entries.len() {
        1..=2 => 8,
        3..=4 => 4,
        5..=16 => 2,
        _ => unreachable!("palette is bounded before packing"),
    };
    let indexed_width = width.div_ceil(indices_per_pixel);
    let height = indices.len() / width;
    let indexed_len = indexed_width
        .checked_mul(height)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(EncodeError::output_size_overflow)?;
    let mut indexed_rgba = Vec::new();
    indexed_rgba
        .try_reserve_exact(indexed_len)
        .map_err(|_| EncodeError::allocation_failed())?;
    let bits_per_index = 8 / indices_per_pixel;
    for row in indices.chunks_exact(width) {
        for packed_indices in row.chunks(indices_per_pixel) {
            let mut packed = 0_u8;
            for (position, index) in packed_indices.iter().copied().enumerate() {
                packed |= index << (position * bits_per_index);
            }
            indexed_rgba.extend_from_slice(&[0, packed, 0, 0]);
        }
    }
    Ok(Some(PalettePlan {
        entries,
        indexed_rgba,
        indexed_width,
    }))
}

fn write_palette_image(writer: &mut BitWriter, entries: &[[u8; 4]]) -> Result<(), EncodeError> {
    write_literal_entropy_image_prefix(writer, false)?;
    let mut previous = [0_u8; 4];
    for (index, entry) in entries.iter().copied().enumerate() {
        let delta = if index == 0 {
            entry
        } else {
            [
                entry[0].wrapping_sub(previous[0]),
                entry[1].wrapping_sub(previous[1]),
                entry[2].wrapping_sub(previous[2]),
                entry[3].wrapping_sub(previous[3]),
            ]
        };
        for channel in [delta[1], delta[0], delta[2], delta[3]] {
            write_fixed_symbol(writer, channel)?;
        }
        previous = entry;
    }
    Ok(())
}

/// Writes the predictor's transform subimage. Transform subimages are not
/// level zero, so they omit the main image's meta-Huffman flag.
fn write_predictor_mode_image(
    writer: &mut BitWriter,
    width: u32,
    height: u32,
) -> Result<(), EncodeError> {
    write_literal_entropy_image_prefix(writer, false)?;
    let mode_width = width.div_ceil(PREDICTOR_BLOCK_SIZE);
    let mode_height = height.div_ceil(PREDICTOR_BLOCK_SIZE);
    let mode_pixels = u64::from(mode_width)
        .checked_mul(u64::from(mode_height))
        .ok_or_else(EncodeError::input_size_overflow)?;
    for _ in 0..mode_pixels {
        // Predictor mode is carried in green; all transform-image channels
        // still use the ordinary literal entropy syntax.
        for channel in [LEFT_PREDICTOR_MODE, 0, 0, u8::MAX] {
            write_fixed_symbol(writer, channel)?;
        }
    }
    Ok(())
}

/// Writes a single, global VP8L color-transform table. A seven-bit block size
/// makes the table one pixel for images up to 128 by 128, while still keeping
/// its dimensions and edge behaviour valid for all VP8L image sizes.
fn write_color_transform_image(
    writer: &mut BitWriter,
    width: u32,
    height: u32,
    plan: ColorTransformPlan,
) -> Result<(), EncodeError> {
    write_literal_entropy_image_prefix(writer, false)?;
    let block_size = 1_u32 << COLOR_TRANSFORM_BLOCK_BITS;
    let block_width = width.div_ceil(block_size);
    let block_height = height.div_ceil(block_size);
    let pixels = u64::from(block_width)
        .checked_mul(u64::from(block_height))
        .ok_or_else(EncodeError::input_size_overflow)?;
    for _ in 0..pixels {
        // VP8L stores green-to-red in blue, green-to-blue in green, and
        // red-to-blue in red. The alpha transform-image channel is unused.
        for channel in [
            plan.green_to_blue as u8,
            plan.red_to_blue as u8,
            plan.green_to_red as u8,
            0,
        ] {
            write_fixed_symbol(writer, channel)?;
        }
    }
    Ok(())
}

pub(crate) fn collect_entropy_tokens(
    rgba: &[u8],
    width: usize,
    use_subtract_green: bool,
    use_left_predictor: bool,
    color_cache_bits: u8,
) -> Result<(Vec<EntropyToken>, EntropyFrequencies), EncodeError> {
    let mut tokens = Vec::new();
    tokens
        .try_reserve_exact(rgba.len() / 4)
        .map_err(|_| EncodeError::allocation_failed())?;
    let mut frequencies = EntropyFrequencies {
        green: [0; MAIN_GREEN_ALPHABET_SIZE],
        green_len: GREEN_ALPHABET_SIZE + color_cache_size(color_cache_bits),
        red: [0; CHANNEL_ALPHABET_SIZE],
        blue: [0; CHANNEL_ALPHABET_SIZE],
        alpha: [0; CHANNEL_ALPHABET_SIZE],
        distance: [0; DISTANCE_ALPHABET_SIZE],
    };
    let mut color_cache = [0_u32; MAX_COLOR_CACHE_SIZE];
    let mut residuals = Vec::new();
    residuals
        .try_reserve_exact(rgba.len() / 4)
        .map_err(|_| EncodeError::allocation_failed())?;
    for (index, _) in rgba.chunks_exact(4).enumerate() {
        let current = if use_subtract_green {
            subtract_green_pixel(rgba, index)
        } else {
            pixel_at(rgba, index)
        };
        let predictor = if use_left_predictor {
            left_predictor(rgba, index, width)
        } else {
            [0; 4]
        };
        let residual = [
            current[0].wrapping_sub(predictor[0]),
            current[1].wrapping_sub(predictor[1]),
            current[2].wrapping_sub(predictor[2]),
            current[3].wrapping_sub(predictor[3]),
        ];
        residuals.push(residual);
    }

    let mut index = 0_usize;
    while index < residuals.len() {
        let residual = residuals[index];
        if index != 0 && residual == residuals[index - 1] {
            let mut length = 1_usize;
            while length < 4096
                && index + length < residuals.len()
                && residuals[index + length] == residual
            {
                length += 1;
            }
            if length >= 3 {
                let (length_prefix, _) = vp8l_prefix(length, 24)?;
                increment_frequency(
                    &mut frequencies.green,
                    CHANNEL_ALPHABET_SIZE + length_prefix,
                )?;
                let (distance_prefix, _) = vp8l_prefix(121, DISTANCE_ALPHABET_SIZE)?;
                increment_frequency(&mut frequencies.distance, distance_prefix)?;
                for _ in 0..length {
                    update_color_cache(&mut color_cache, color_cache_bits, pack_argb(residual));
                }
                tokens.push(EntropyToken::Copy { length });
                index += length;
                continue;
            }
        }
        let color = pack_argb(residual);
        let cache_index = if color_cache_bits == 0 {
            0
        } else {
            color_cache_index(color, color_cache_bits)
        };
        if color_cache_bits != 0 && color_cache[cache_index] == color {
            increment_frequency(&mut frequencies.green, FIRST_CACHE_SYMBOL + cache_index)?;
            tokens.push(EntropyToken::Cache(cache_index));
        } else {
            increment_frequency(&mut frequencies.green, usize::from(residual[1]))?;
            increment_frequency(&mut frequencies.red, usize::from(residual[0]))?;
            increment_frequency(&mut frequencies.blue, usize::from(residual[2]))?;
            increment_frequency(&mut frequencies.alpha, usize::from(residual[3]))?;
            tokens.push(EntropyToken::Literal(residual));
        }
        color_cache[cache_index] = color;
        index += 1;
    }
    Ok((tokens, frequencies))
}

fn increment_frequency(table: &mut [u32], symbol: usize) -> Result<(), EncodeError> {
    let frequency = table
        .get_mut(symbol)
        .ok_or_else(EncodeError::output_size_overflow)?;
    *frequency = frequency
        .checked_add(1)
        .ok_or_else(EncodeError::output_size_overflow)?;
    Ok(())
}

fn write_main_entropy_image_prefix(
    writer: &mut BitWriter,
    frequencies: &EntropyFrequencies,
    color_cache_bits: u8,
) -> Result<EntropyTables, EncodeError> {
    write_bits(writer, u32::from(color_cache_bits != 0), 1)?;
    if color_cache_bits != 0 {
        write_bits(writer, u32::from(color_cache_bits), 4)?;
    }
    write_bits(writer, 0, 1)?; // One entropy-code group, not meta-Huffman.
    let green = write_adaptive_table(writer, &frequencies.green[..frequencies.green_len])?;
    let red = write_adaptive_table(writer, &frequencies.red)?;
    let blue = write_adaptive_table(writer, &frequencies.blue)?;
    let alpha = write_adaptive_table(writer, &frequencies.alpha)?;
    let distance = write_adaptive_table(writer, &frequencies.distance)?;
    Ok(EntropyTables {
        green,
        red,
        blue,
        alpha,
        distance,
    })
}

fn write_literal_entropy_image_prefix(
    writer: &mut BitWriter,
    is_level_zero: bool,
) -> Result<(), EncodeError> {
    write_bits(writer, 0, 1)?; // No color cache.
    if is_level_zero {
        write_bits(writer, 0, 1)?; // One entropy-code group, not meta-Huffman.
    }
    write_literal_table(writer, GREEN_ALPHABET_SIZE, 256)?;
    write_literal_table(writer, CHANNEL_ALPHABET_SIZE, CHANNEL_ALPHABET_SIZE)?;
    write_literal_table(writer, CHANNEL_ALPHABET_SIZE, CHANNEL_ALPHABET_SIZE)?;
    write_literal_table(writer, CHANNEL_ALPHABET_SIZE, CHANNEL_ALPHABET_SIZE)?;
    Ok(write_simple_table(writer, 0)?) // Distance codes are unused.
}

/// Writes a deterministic, frequency-adaptive complete Huffman table.
///
/// Symbols with greater observed frequency receive the shortest lengths in a
/// balanced complete tree. This intentionally bounded first M6 form avoids a
/// search heuristic while producing valid canonical codes of at most nine bits
/// for every current VP8L alphabet.
fn write_adaptive_table(
    writer: &mut BitWriter,
    frequencies: &[u32],
) -> Result<EncodingTable, EncodeError> {
    let (lengths, table) = prepare_adaptive_table(frequencies)?;
    write_normal_table(writer, &lengths)?;
    Ok(table)
}

fn prepare_adaptive_table(frequencies: &[u32]) -> Result<(Vec<u8>, EncodingTable), EncodeError> {
    let mut ranked = frequencies
        .iter()
        .copied()
        .enumerate()
        .filter_map(|(symbol, frequency)| (frequency != 0).then_some((frequency, symbol)))
        .collect::<Vec<_>>();
    if ranked.is_empty() {
        ranked.push((1, 0));
    }
    ranked.sort_unstable_by(
        |(left_frequency, left_symbol), (right_frequency, right_symbol)| {
            right_frequency
                .cmp(left_frequency)
                .then_with(|| left_symbol.cmp(right_symbol))
        },
    );

    let mut lengths = vec![0_u8; frequencies.len()];
    if ranked.len() == 1 {
        let symbol = ranked[0].1;
        // A simple code cannot express VP8L's green cache symbols above 255.
        // The normal one-symbol form is valid for every alphabet and decodes
        // without consuming a data bit.
        lengths[symbol] = 1;
        let table = canonical_table(&lengths)?;
        return Ok((lengths, table));
    }

    let floor_log = usize::BITS - 1 - ranked.len().leading_zeros();
    let base = 1_usize << floor_log;
    let short_count = base
        .checked_mul(2)
        .and_then(|count| count.checked_sub(ranked.len()))
        .ok_or_else(EncodeError::output_size_overflow)?;
    for (rank, (_, symbol)) in ranked.iter().enumerate() {
        lengths[*symbol] = if rank < short_count {
            floor_log as u8
        } else {
            floor_log as u8 + 1
        };
    }
    let table = canonical_table(&lengths)?;
    Ok((lengths, table))
}

fn write_normal_table(writer: &mut BitWriter, lengths: &[u8]) -> Result<(), EncodeError> {
    write_bits(writer, 0, 1)?; // Normal Huffman-code representation.
    write_bits(writer, 15, 4)?; // All nineteen code-length-code entries.
    for symbol in CODE_LENGTH_CODE_ORDER {
        write_bits(writer, if symbol <= 15 { 4 } else { 0 }, 3)?;
    }
    write_bits(writer, 0, 1)?; // No max-code-length-symbol shortening.
    for &length in lengths {
        write_canonical_symbol(writer, u32::from(length), 4)?;
    }
    Ok(())
}

/// Emits one bounded distance-one VP8L copy. The distance code `121` is the
/// format's linear representation of scan-line distance one, avoiding the
/// spatial-distance map while remaining valid at every image width.
fn write_lz77_copy(
    writer: &mut BitWriter,
    tables: &EntropyTables,
    length: usize,
) -> Result<(), EncodeError> {
    let (length_prefix, length_extra) = vp8l_prefix(length, 24)?;
    write_table_symbol(writer, &tables.green, CHANNEL_ALPHABET_SIZE + length_prefix)?;
    write_bits(writer, length_extra.0, length_extra.1)?;
    let (distance_prefix, distance_extra) = vp8l_prefix(121, DISTANCE_ALPHABET_SIZE)?;
    write_table_symbol(writer, &tables.distance, distance_prefix)?;
    write_bits(writer, distance_extra.0, distance_extra.1)
}

/// Applies VP8L's forward subtract-green transform to one input pixel.
fn subtract_green_pixel(rgba: &[u8], index: usize) -> [u8; 4] {
    let [red, green, blue, alpha] = pixel_at(rgba, index);
    [
        red.wrapping_sub(green),
        green,
        blue.wrapping_sub(green),
        alpha,
    ]
}

/// Evaluates a bounded deterministic coefficient set. The transform's table
/// costs a full nested entropy image, so it is only considered for substantial
/// images and must reduce the signed channel-residual score by at least 25%.
pub(crate) fn select_color_transform(rgba: &[u8]) -> Option<ColorTransformPlan> {
    if rgba.len() / 4 < MIN_COLOR_TRANSFORM_PIXELS {
        return None;
    }
    const CANDIDATES: [ColorTransformPlan; 6] = [
        ColorTransformPlan {
            green_to_red: 32,
            green_to_blue: 32,
            red_to_blue: 0,
        },
        ColorTransformPlan {
            green_to_red: 32,
            green_to_blue: 0,
            red_to_blue: 32,
        },
        ColorTransformPlan {
            green_to_red: 0,
            green_to_blue: 32,
            red_to_blue: 32,
        },
        ColorTransformPlan {
            green_to_red: 48,
            green_to_blue: 48,
            red_to_blue: 0,
        },
        ColorTransformPlan {
            green_to_red: -32,
            green_to_blue: -32,
            red_to_blue: 0,
        },
        ColorTransformPlan {
            green_to_red: 64,
            green_to_blue: 64,
            red_to_blue: 0,
        },
    ];
    let baseline = color_residual_score(rgba, None);
    let mut selected = None;
    let mut best = baseline;
    for candidate in CANDIDATES {
        let score = color_residual_score(rgba, Some(candidate));
        if score < best {
            best = score;
            selected = Some(candidate);
        }
    }
    (best.saturating_mul(4) <= baseline.saturating_mul(3)).then_some(selected?)
}

/// Applies the forward form of VP8L's color transform. Blue must use the
/// original red channel because the decoder uses reconstructed red for its
/// inverse step.
fn apply_forward_color_transform(
    rgba: &[u8],
    plan: ColorTransformPlan,
) -> Result<Vec<u8>, EncodeError> {
    let mut transformed = Vec::new();
    transformed
        .try_reserve_exact(rgba.len())
        .map_err(|_| EncodeError::allocation_failed())?;
    for pixel in rgba.chunks_exact(4) {
        let red_delta = color_transform_delta(pixel[1], plan.green_to_red);
        let blue_delta = color_transform_delta(pixel[1], plan.green_to_blue)
            + color_transform_delta(pixel[0], plan.red_to_blue);
        transformed.extend_from_slice(&[
            pixel[0].wrapping_sub(red_delta as u8),
            pixel[1],
            pixel[2].wrapping_sub(blue_delta as u8),
            pixel[3],
        ]);
    }
    Ok(transformed)
}

fn color_transform_delta(channel: u8, multiplier: i8) -> i16 {
    (i16::from(channel as i8) * i16::from(multiplier)) >> 5
}

/// Scores the signed size of color residuals after the candidate transform.
/// It intentionally avoids an entropy-model feedback loop; the strict 25%
/// threshold keeps this bounded estimator from paying for a transform table on
/// weak correlations.
fn color_residual_score(rgba: &[u8], plan: Option<ColorTransformPlan>) -> u64 {
    rgba.chunks_exact(4)
        .map(|pixel| {
            let (red, blue) = if let Some(plan) = plan {
                (
                    pixel[0].wrapping_sub(color_transform_delta(pixel[1], plan.green_to_red) as u8),
                    pixel[2].wrapping_sub(
                        (color_transform_delta(pixel[1], plan.green_to_blue)
                            + color_transform_delta(pixel[0], plan.red_to_blue))
                            as u8,
                    ),
                )
            } else {
                (pixel[0], pixel[2])
            };
            u64::from(signed_byte_magnitude(red)) + u64::from(signed_byte_magnitude(blue))
        })
        .sum()
}

fn signed_byte_magnitude(value: u8) -> u8 {
    let signed = value as i8;
    signed.unsigned_abs()
}

fn pixel_at(rgba: &[u8], index: usize) -> [u8; 4] {
    let offset = index * 4;
    [
        rgba[offset],
        rgba[offset + 1],
        rgba[offset + 2],
        rgba[offset + 3],
    ]
}

/// Returns VP8L's fixed left-predictor value for the subtract-green image.
/// Boundary rules are defined by the VP8L format, not by predictor mode.
fn left_predictor(rgba: &[u8], index: usize, width: usize) -> [u8; 4] {
    if index == 0 {
        return [0, 0, 0, u8::MAX];
    }
    let x = index % width;
    let predictor_index = if x == 0 { index - width } else { index - 1 };
    subtract_green_pixel(rgba, predictor_index)
}

pub(crate) fn validate_input(width: u32, height: u32, rgba: &[u8]) -> Result<(), EncodeError> {
    if width == 0 || height == 0 || width > MAX_DIMENSION || height > MAX_DIMENSION {
        return Err(EncodeError::invalid_dimensions());
    }
    let expected = usize::try_from(u64::from(width) * u64::from(height))
        .ok()
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(EncodeError::input_size_overflow)?;
    if rgba.len() != expected {
        return Err(EncodeError::invalid_rgba_length());
    }
    Ok(())
}

/// Writes a normal table containing `used_symbols` fixed eight-bit symbols.
/// The remainder of `alphabet_size` is unused. A 256-symbol literal alphabet
/// is therefore complete, while VP8L's extra green symbols remain absent.
fn write_literal_table(
    writer: &mut BitWriter,
    alphabet_size: usize,
    used_symbols: usize,
) -> Result<(), EncodeError> {
    debug_assert_eq!(used_symbols, 256);
    debug_assert!(used_symbols <= alphabet_size);

    write_bits(writer, 0, 1)?; // Normal Huffman-code representation.
    write_bits(writer, 8, 4)?; // 4 + 8 = 12 code-length-code entries.
    // In VP8L wire order these entries describe symbols
    // 17, 18, 0, 1, 2, 3, 4, 5, 16, 6, 7, and 8. Only 0 and 8 are needed;
    // their two one-bit codes form a complete code-length alphabet.
    for length in [0_u32, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 1] {
        write_bits(writer, length, 3)?;
    }
    write_bits(writer, 0, 1)?; // No max-code-length-symbol shortening.
    for symbol in 0..alphabet_size {
        write_bits(writer, u32::from(symbol < used_symbols), 1)?;
    }
    Ok(())
}

/// Emits one symbol from a fixed eight-bit canonical table. VP8L transmits
/// canonical codes least-significant bit first, hence the bit reversal.
fn write_fixed_symbol(writer: &mut BitWriter, symbol: u8) -> Result<(), EncodeError> {
    Ok(write_canonical_symbol(writer, u32::from(symbol), 8)?)
}

fn pack_argb(rgba: [u8; 4]) -> u32 {
    (u32::from(rgba[3]) << 24)
        | (u32::from(rgba[0]) << 16)
        | (u32::from(rgba[1]) << 8)
        | u32::from(rgba[2])
}

pub(crate) fn select_color_cache_bits(
    rgba: &[u8],
    width: usize,
    use_subtract_green: bool,
    use_left_predictor: bool,
) -> u8 {
    let mut selected_bits = 0;
    let mut best_hits = 0_u32;
    for bits in 1..=MAX_ENCODER_COLOR_CACHE_BITS {
        let mut cache = [0_u32; MAX_COLOR_CACHE_SIZE];
        let mut hits = 0_u32;
        for (index, _) in rgba.chunks_exact(4).enumerate() {
            let current = if use_subtract_green {
                subtract_green_pixel(rgba, index)
            } else {
                pixel_at(rgba, index)
            };
            let predictor = if use_left_predictor {
                left_predictor(rgba, index, width)
            } else {
                [0; 4]
            };
            let residual = [
                current[0].wrapping_sub(predictor[0]),
                current[1].wrapping_sub(predictor[1]),
                current[2].wrapping_sub(predictor[2]),
                current[3].wrapping_sub(predictor[3]),
            ];
            let color = pack_argb(residual);
            let cache_index = color_cache_index(color, bits);
            if cache[cache_index] == color {
                hits = hits.saturating_add(1);
            }
            cache[cache_index] = color;
        }
        if hits > best_hits {
            best_hits = hits;
            selected_bits = bits;
        }
    }
    selected_bits
}

/// Keeps the fixed left mode only when it creates a material number of exact
/// transformed-neighbour matches. Otherwise omitting the predictor avoids its
/// nested transform image and makes the entropy stream directly represent the
/// subtract-green samples.
pub(crate) fn select_left_predictor(rgba: &[u8], width: usize) -> bool {
    let mut matching_neighbours = 0_usize;
    for index in 1..rgba.len() / 4 {
        let current = subtract_green_pixel(rgba, index);
        let predictor = left_predictor(rgba, index, width);
        if current == predictor {
            matching_neighbours += 1;
        }
    }
    matching_neighbours.saturating_mul(16) >= rgba.len() / 4
}

const fn color_cache_size(bits: u8) -> usize {
    if bits == 0 { 0 } else { 1 << bits }
}

fn color_cache_index(color: u32, bits: u8) -> usize {
    debug_assert!(bits != 0 && bits <= MAX_ENCODER_COLOR_CACHE_BITS);
    hash_color(color, bits)
}

fn update_color_cache(cache: &mut [u32; MAX_COLOR_CACHE_SIZE], bits: u8, color: u32) {
    if bits != 0 {
        cache[color_cache_index(color, bits)] = color;
    }
}

fn write_bits(writer: &mut BitWriter, value: u32, count: u8) -> Result<(), EncodeError> {
    writer
        .write_bits(value, count)
        .map_err(|_| EncodeError::allocation_failed())
}

fn wrap_vp8l(payload: Vec<u8>) -> Result<Vec<u8>, EncodeError> {
    webp_container::serialize_vp8l(payload, 0, 0, false, webp_container::Metadata::default())
        .map_err(|error| match error.kind() {
            webp_container::ContainerErrorKind::SizeOverflow => EncodeError::output_size_overflow(),
            webp_container::ContainerErrorKind::AllocationFailed => {
                EncodeError::allocation_failed()
            }
            webp_container::ContainerErrorKind::InvalidDimensions => {
                EncodeError::invalid_dimensions()
            }
            webp_container::ContainerErrorKind::InvalidAnimation => {
                EncodeError::invalid_animation()
            }
            webp_container::ContainerErrorKind::UnexpectedEof
            | webp_container::ContainerErrorKind::InvalidContainer
            | webp_container::ContainerErrorKind::LimitExceeded => {
                EncodeError::output_size_overflow()
            }
        })
}
