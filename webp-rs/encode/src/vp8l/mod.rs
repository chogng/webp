//! VP8L lossless encoding and spatial entropy planning.
//!
//! This M6 slice writes a single entropy group with reversible color,
//! subtract-green, and predictor transforms, a bounded color cache, and
//! distance-one backward references. Entropy tables use deterministic
//! frequency-ranked balanced Huffman codes. Small palette images use color
//! indexing.

use crate::EncodeError;
use webp_utils::BitWriter;

use self::huffman::EncodingTable;
use self::huffman::canonical_table;
use self::huffman::write_canonical_symbol;
use self::huffman::write_simple_table;
#[cfg(test)]
use self::huffman::write_table_symbol;
use self::packet_sink::PackedTokenWriter;
use self::prefix::encode_prefix as vp8l_prefix;
#[cfg(test)]
use self::token_stream::EntropyFrequencies;
pub(crate) use self::token_stream::EntropyToken;
pub(crate) use self::token_stream::TokenStream;
pub(crate) use self::token_stream::select_color_cache_bits;
pub(crate) use self::token_stream::select_left_predictor;
use self::token_stream::{
    CHANNEL_ALPHABET_SIZE, DISTANCE_ALPHABET_SIZE, FIRST_CACHE_SYMBOL, GREEN_ALPHABET_SIZE,
};
use self::token_stream::{ParseMode, ResidualImage};

pub(crate) const MAX_DIMENSION: u32 = 1 << 14;
const SIGNATURE: u8 = 0x2f;

const PREDICTOR_BLOCK_SIZE: u32 = 4;
const LEFT_PREDICTOR_MODE: u8 = 1;
const CODE_LENGTH_CODE_ORDER: [usize; 19] = [
    17, 18, 0, 1, 2, 3, 4, 5, 16, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15,
];
pub const COLOR_TRANSFORM_BLOCK_BITS: u8 = 7;

mod entropy_plan;
pub(crate) mod high_compression;
pub(crate) mod huffman;
mod lz77;
mod packet_sink;
mod portfolio_policy;
mod predictor_plan;
mod prefix;
mod source_analysis;
mod token_stream;

pub struct EntropyTables {
    green: EncodingTable,
    red: EncodingTable,
    blue: EncodingTable,
    alpha: EncodingTable,
    distance: EncodingTable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ColorTransformPlan {
    pub green_to_red: i8,
    pub green_to_blue: i8,
    pub red_to_blue: i8,
}

#[path = "spatial_cluster.rs"]
mod spatial_cluster;
#[path = "spatial_plan.rs"]
pub(crate) mod spatial_plan;
#[path = "spatial_writer.rs"]
pub(crate) mod spatial_writer;

#[cfg(test)]
#[path = "coarse_spatial_tests.rs"]
mod coarse_spatial_tests;
// The product reproducer is coupled to writer invariants and is intentionally
// compiled within the VP8L writer module.
#[cfg(test)]
#[path = "product_benchmark_tests.rs"]
mod product_benchmark_tests;

pub fn encode_vp8l_payload(
    width: u32,
    height: u32,
    rgba: &[u8],
) -> Result<(Vec<u8>, bool), EncodeError> {
    validate_input(width, height, rgba)?;
    let width_usize = usize::try_from(width).map_err(|_| EncodeError::input_size_overflow())?;
    let analysis = source_analysis::SourceAnalysis::collect(rgba, width_usize)?;
    let facts = analysis.facts();
    if facts.width() != width_usize
        || facts.height()
            != usize::try_from(height).map_err(|_| EncodeError::input_size_overflow())?
        || facts.pixels()
            != width_usize
                .checked_mul(facts.height())
                .ok_or_else(EncodeError::input_size_overflow)?
        || facts.identity().rgba_bytes() != rgba.len()
    {
        return Err(EncodeError::output_size_overflow());
    }
    let has_alpha = facts.has_alpha();
    let color_transform = analysis.color_transform();
    if facts.palette_colors().is_some_and(|colors| colors <= 16) {
        if let Some(palette) = analysis.into_palette() {
            return encode_palette_vp8l_payload(width, height, has_alpha, palette, false);
        }
    }
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
    let stream = TokenStream::collect(
        &transformed,
        width_usize,
        true,
        use_left_predictor,
        color_cache_bits,
    )?;
    let plan = entropy_plan::EntropyPlan::build_for_stream(stream.statistics())?;
    plan.write_main_prefix(&mut bits, color_cache_bits)?;
    Ok((
        write_packed_tokens(bits, stream.tokens(), &plan)?,
        has_alpha,
    ))
}

pub(super) fn encode_palette_vp8l_payload(
    width: u32,
    height: u32,
    has_alpha: bool,
    palette: source_analysis::PalettePlan,
    compact: bool,
) -> Result<(Vec<u8>, bool), EncodeError> {
    let mut bits = BitWriter::new();
    write_vp8l_header(&mut bits, width, height, has_alpha)?;
    write_bits(&mut bits, 1, 1)?; // Color-indexing transform follows.
    write_bits(&mut bits, 3, 2)?;
    write_bits(
        &mut bits,
        u32::try_from(palette.entries().len() - 1)
            .map_err(|_| EncodeError::output_size_overflow())?,
        8,
    )?;
    write_palette_image(&mut bits, palette.entries(), compact)?;
    write_bits(&mut bits, 0, 1)?; // Transform-list terminator.

    let selected_cache_bits = select_color_cache_bits(
        palette.indexed_rgba(),
        palette.indexed_width(),
        false,
        false,
    );
    let (stream, plan) = if compact {
        let residuals = ResidualImage::collect_with_predictor(
            palette.indexed_rgba(),
            palette.indexed_width(),
            false,
            None,
            &predictor_plan::PredictorPlan::None,
        )?;
        let mut best = None;
        for color_cache_bits in [0, selected_cache_bits]
            .into_iter()
            .take(if selected_cache_bits == 0 { 1 } else { 2 })
        {
            for parse_mode in [ParseMode::Greedy, ParseMode::LazyDeep] {
                let stream = TokenStream::collect_compressed_with_spatial(
                    &residuals,
                    color_cache_bits,
                    parse_mode,
                )?;
                let plan =
                    entropy_plan::EntropyPlan::build_compact_for_stream(stream.statistics())?;
                let encoded_bits = plan.main_bits(color_cache_bits)?;
                if best
                    .as_ref()
                    .is_none_or(|(_, _, best_bits)| encoded_bits < *best_bits)
                {
                    best = Some((stream, plan, encoded_bits));
                }
            }
        }
        let (stream, plan, _) = best.ok_or_else(EncodeError::output_size_overflow)?;
        (stream, plan)
    } else {
        let stream = TokenStream::collect(
            palette.indexed_rgba(),
            palette.indexed_width(),
            false,
            false,
            selected_cache_bits,
        )?;
        let plan = entropy_plan::EntropyPlan::build_for_stream(stream.statistics())?;
        (stream, plan)
    };
    plan.write_main_prefix(&mut bits, stream.color_cache_bits())?;
    Ok((
        write_packed_tokens(bits, stream.tokens(), &plan)?,
        has_alpha,
    ))
}

fn write_packed_tokens(
    bits: BitWriter,
    tokens: &[EntropyToken],
    plan: &entropy_plan::EntropyPlan,
) -> Result<Vec<u8>, EncodeError> {
    let mut packed = PackedTokenWriter::from_prefix(bits, plan.token_bits())?;
    for &token in tokens {
        packed.write_token(token, plan.tables())?;
    }
    packed.finish()
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

#[cfg(test)]
pub(crate) fn try_make_palette_plan(
    rgba: &[u8],
    width: usize,
) -> Result<Option<source_analysis::PalettePlan>, EncodeError> {
    Ok(source_analysis::SourceAnalysis::collect(rgba, width)?.into_palette())
}

fn write_palette_image(
    writer: &mut BitWriter,
    entries: &[[u8; 4]],
    compact: bool,
) -> Result<(), EncodeError> {
    let mut deltas = Vec::new();
    deltas
        .try_reserve_exact(
            entries
                .len()
                .checked_mul(4)
                .ok_or_else(EncodeError::output_size_overflow)?,
        )
        .map_err(|_| EncodeError::allocation_failed())?;
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
        deltas.extend_from_slice(&delta);
        previous = entry;
    }
    if compact {
        write_compact_entropy_image(writer, &deltas, entries.len())
    } else {
        write_literal_rgba_entropy_image(writer, &deltas)
    }
}

pub(super) fn write_compact_entropy_image(
    writer: &mut BitWriter,
    rgba: &[u8],
    width: usize,
) -> Result<(), EncodeError> {
    let stream = TokenStream::collect(rgba, width, false, false, 0)?;
    let plan = entropy_plan::EntropyPlan::build_compact_for_stream(stream.statistics())?;
    let compressed_bits = 1_usize
        .checked_add(plan.encoded_bits()?)
        .ok_or_else(EncodeError::output_size_overflow)?;
    let mut literal = BitWriter::new();
    write_literal_rgba_entropy_image(&mut literal, rgba)?;
    if compressed_bits >= literal.bit_len() {
        return write_literal_rgba_entropy_image(writer, rgba);
    }
    write_bits(writer, 0, 1)?;
    plan.write_tables(writer)?;
    let prefix = std::mem::take(writer);
    let mut packed = PackedTokenWriter::from_prefix(prefix, plan.token_bits())?;
    for &token in stream.tokens() {
        packed.write_token(token, plan.tables())?;
    }
    *writer = packed.into_prefix()?;
    Ok(())
}

fn write_literal_rgba_entropy_image(
    writer: &mut BitWriter,
    rgba: &[u8],
) -> Result<(), EncodeError> {
    write_literal_entropy_image_prefix(writer, false)?;
    for pixel in rgba.chunks_exact(4) {
        for channel in [pixel[1], pixel[0], pixel[2], pixel[3]] {
            write_fixed_symbol(writer, channel)?;
        }
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
#[cfg(test)]
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
#[cfg(test)]
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

/// Evaluates a bounded deterministic coefficient set. The transform's table
/// costs a full nested entropy image, so it is only considered for substantial
/// images and must reduce the signed channel-residual score by at least 25%.
#[cfg(test)]
pub(crate) fn select_color_transform(rgba: &[u8]) -> Option<ColorTransformPlan> {
    source_analysis::select_color_transform(rgba)
}

fn apply_forward_color_transform(
    rgba: &[u8],
    plan: ColorTransformPlan,
) -> Result<Vec<u8>, EncodeError> {
    let mut transformed = Vec::new();
    transformed
        .try_reserve_exact(rgba.len())
        .map_err(|_| EncodeError::allocation_failed())?;
    for pixel in rgba.chunks_exact(4) {
        transformed.extend_from_slice(&source_analysis::forward_color_pixel(pixel, plan));
    }
    Ok(transformed)
}

pub fn validate_input(width: u32, height: u32, rgba: &[u8]) -> Result<(), EncodeError> {
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

fn write_bits(writer: &mut BitWriter, value: u32, count: u8) -> Result<(), EncodeError> {
    writer
        .write_bits(value, count)
        .map_err(|_| EncodeError::allocation_failed())
}

fn wrap_vp8l(payload: Vec<u8>) -> Result<Vec<u8>, EncodeError> {
    webp_mux::serialize_vp8l(payload, 0, 0, false, webp_mux::Metadata::default()).map_err(|error| {
        match error.kind() {
            webp_mux::ContainerErrorKind::SizeOverflow => EncodeError::output_size_overflow(),
            webp_mux::ContainerErrorKind::AllocationFailed => EncodeError::allocation_failed(),
            webp_mux::ContainerErrorKind::InvalidDimensions => EncodeError::invalid_dimensions(),
            webp_mux::ContainerErrorKind::InvalidAnimation => EncodeError::invalid_animation(),
            webp_mux::ContainerErrorKind::UnexpectedEof
            | webp_mux::ContainerErrorKind::InvalidContainer
            | webp_mux::ContainerErrorKind::LimitExceeded => EncodeError::output_size_overflow(),
        }
    })
}
