//! Standard VP8L spatial-map serialization and complete-file selection.

use super::single_plan::SinglePlan;
use super::spatial_cluster::token_span;
use super::spatial_packet_writer::PackedTokenWriter;
use super::spatial_plan::{SpatialPlan, SpatialProfile};
use super::{
    BitWriter, EncodeError, EntropyFrequencies, EntropyTables, EntropyToken, FIRST_CACHE_SYMBOL,
    collect_entropy_tokens, validate_input, wrap_vp8l, write_adaptive_table, write_bits,
    write_lz77_copy, write_main_entropy_image_prefix, write_table_symbol, write_vp8l_header,
};

struct Prepared {
    width: usize,
    height: usize,
    width_u32: u32,
    height_u32: u32,
    has_alpha: bool,
    tokens: Vec<EntropyToken>,
    frequencies: EntropyFrequencies,
}

pub(crate) fn encode_profile(
    width: u32,
    height: u32,
    rgba: &[u8],
    profile: SpatialProfile,
) -> Result<Vec<u8>, EncodeError> {
    let prepared = prepare(width, height, rgba)?;
    encode_prepared(&prepared, profile).map(|(encoded, _)| encoded)
}

fn encode_profile_control(
    prepared: &Prepared,
    profile: SpatialProfile,
) -> Result<Vec<u8>, EncodeError> {
    let single = encode_single(prepared)?;
    let candidate = encode_spatial(prepared, profile)?;
    Ok(if candidate.len() < single.len() {
        candidate
    } else {
        single
    })
}

#[derive(Clone, Copy)]
enum SelectionKind {
    Candidate,
    Single,
    Fallback,
}

fn encode_prepared(
    prepared: &Prepared,
    profile: SpatialProfile,
) -> Result<(Vec<u8>, SelectionKind), EncodeError> {
    encode_prepared_with_plan(prepared, profile, SinglePlan::build(&prepared.frequencies))
}

fn encode_prepared_with_plan(
    prepared: &Prepared,
    profile: SpatialProfile,
    plan: Result<SinglePlan, EncodeError>,
) -> Result<(Vec<u8>, SelectionKind), EncodeError> {
    let plan = match plan {
        Ok(plan) => plan,
        Err(_) => {
            return encode_profile_control(prepared, profile)
                .map(|encoded| (encoded, SelectionKind::Fallback));
        }
    };
    let candidate = encode_spatial(prepared, profile)?;
    if candidate_wins(candidate.len(), plan.riff_bytes()) {
        Ok((candidate, SelectionKind::Candidate))
    } else {
        encode_single_with_plan(prepared, &plan).map(|encoded| (encoded, SelectionKind::Single))
    }
}

pub(crate) const fn candidate_wins(candidate_bytes: usize, single_bytes: usize) -> bool {
    candidate_bytes < single_bytes
}

fn prepare(width: u32, height: u32, rgba: &[u8]) -> Result<Prepared, EncodeError> {
    validate_input(width, height, rgba)?;
    let width_usize = usize::try_from(width).map_err(|_| EncodeError::input_size_overflow())?;
    let height_usize = usize::try_from(height).map_err(|_| EncodeError::input_size_overflow())?;
    let (tokens, frequencies) = collect_entropy_tokens(rgba, width_usize, true, false, 0)?;
    Ok(Prepared {
        width: width_usize,
        height: height_usize,
        width_u32: width,
        height_u32: height,
        has_alpha: rgba.chunks_exact(4).any(|pixel| pixel[3] != u8::MAX),
        tokens,
        frequencies,
    })
}

fn encode_single(prepared: &Prepared) -> Result<Vec<u8>, EncodeError> {
    let mut bits = BitWriter::new();
    write_fast_prefix(&mut bits, prepared)?;
    let tables = write_main_entropy_image_prefix(&mut bits, &prepared.frequencies, 0)?;
    write_tokens(&mut bits, &prepared.tokens, &tables)?;
    wrap_vp8l(bits.into_bytes())
}

fn encode_single_with_plan(prepared: &Prepared, plan: &SinglePlan) -> Result<Vec<u8>, EncodeError> {
    let mut bits = BitWriter::new();
    write_fast_prefix(&mut bits, prepared)?;
    plan.write_main_prefix(&mut bits)?;
    write_tokens(&mut bits, &prepared.tokens, plan.tables())?;
    debug_assert_eq!(bits.bit_len(), plan.payload_bits());
    wrap_vp8l(bits.into_bytes())
}

fn encode_spatial(prepared: &Prepared, profile: SpatialProfile) -> Result<Vec<u8>, EncodeError> {
    let plan = SpatialPlan::build(
        &prepared.tokens,
        prepared.width,
        prepared.height,
        0,
        profile,
    )?;
    let mut bits = BitWriter::new();
    write_fast_prefix(&mut bits, prepared)?;
    write_bits(&mut bits, 0, 1)?; // No color cache.
    write_bits(&mut bits, 1, 1)?; // Meta-Huffman image follows.
    write_bits(&mut bits, u32::from(profile.wire_block_bits()), 3)?;
    write_group_map(&mut bits, &plan)?;

    let mut tables = Vec::new();
    tables
        .try_reserve_exact(plan.frequencies().len())
        .map_err(|_| EncodeError::allocation_failed())?;
    for frequencies in plan.frequencies() {
        tables.push(write_five_tables(&mut bits, frequencies)?);
    }
    let mut packed = PackedTokenWriter::from_prefix(bits, prepared.tokens.len())?;
    let mut pixel = 0_usize;
    for &token in &prepared.tokens {
        let group = plan.group_for_pixel(pixel);
        packed.write_token(token, &tables[group])?;
        pixel = pixel
            .checked_add(token_span(token))
            .ok_or_else(EncodeError::output_size_overflow)?;
    }
    wrap_vp8l(packed.finish()?)
}

fn write_fast_prefix(bits: &mut BitWriter, prepared: &Prepared) -> Result<(), EncodeError> {
    write_vp8l_header(
        bits,
        prepared.width_u32,
        prepared.height_u32,
        prepared.has_alpha,
    )?;
    write_bits(bits, 1, 1)?; // Subtract-green transform follows.
    write_bits(bits, 2, 2)?;
    write_bits(bits, 0, 1) // Transform-list terminator.
}

fn write_group_map(bits: &mut BitWriter, plan: &SpatialPlan) -> Result<(), EncodeError> {
    let byte_count = plan
        .group_map()
        .len()
        .checked_mul(4)
        .ok_or_else(EncodeError::output_size_overflow)?;
    let mut rgba = Vec::new();
    rgba.try_reserve_exact(byte_count)
        .map_err(|_| EncodeError::allocation_failed())?;
    for &group in plan.group_map() {
        rgba.extend_from_slice(&[0, group, 0, 0]);
    }
    let (tokens, frequencies) = collect_entropy_tokens(&rgba, plan.map_width(), false, false, 0)?;
    write_bits(bits, 0, 1)?; // Nested map has no color cache.
    let tables = write_five_tables(bits, &frequencies)?;
    write_tokens(bits, &tokens, &tables)
}

fn write_five_tables(
    bits: &mut BitWriter,
    frequencies: &EntropyFrequencies,
) -> Result<EntropyTables, EncodeError> {
    Ok(EntropyTables {
        green: write_adaptive_table(bits, &frequencies.green[..frequencies.green_len])?,
        red: write_adaptive_table(bits, &frequencies.red)?,
        blue: write_adaptive_table(bits, &frequencies.blue)?,
        alpha: write_adaptive_table(bits, &frequencies.alpha)?,
        distance: write_adaptive_table(bits, &frequencies.distance)?,
    })
}

fn write_tokens(
    bits: &mut BitWriter,
    tokens: &[EntropyToken],
    tables: &EntropyTables,
) -> Result<(), EncodeError> {
    for &token in tokens {
        write_token(bits, token, tables)?;
    }
    Ok(())
}

fn write_token(
    bits: &mut BitWriter,
    token: EntropyToken,
    tables: &EntropyTables,
) -> Result<(), EncodeError> {
    match token {
        EntropyToken::Cache(index) => {
            write_table_symbol(bits, &tables.green, FIRST_CACHE_SYMBOL + index)
        }
        EntropyToken::Literal(rgba) => {
            write_table_symbol(bits, &tables.green, usize::from(rgba[1]))?;
            write_table_symbol(bits, &tables.red, usize::from(rgba[0]))?;
            write_table_symbol(bits, &tables.blue, usize::from(rgba[2]))?;
            write_table_symbol(bits, &tables.alpha, usize::from(rgba[3]))
        }
        EntropyToken::Copy { length } => write_lz77_copy(bits, tables, length),
    }
}

#[cfg(test)]
pub(crate) fn encode_single_for_test(
    width: u32,
    height: u32,
    rgba: &[u8],
) -> Result<Vec<u8>, EncodeError> {
    encode_single(&prepare(width, height, rgba)?)
}

#[cfg(test)]
pub(crate) fn encode_candidate_for_test(
    width: u32,
    height: u32,
    rgba: &[u8],
    profile: SpatialProfile,
) -> Result<Vec<u8>, EncodeError> {
    let prepared = prepare(width, height, rgba)?;
    encode_spatial(&prepared, profile)
}

#[cfg(test)]
pub(crate) fn encode_profile_control_for_test(
    width: u32,
    height: u32,
    rgba: &[u8],
    profile: SpatialProfile,
) -> Result<Vec<u8>, EncodeError> {
    let prepared = prepare(width, height, rgba)?;
    encode_profile_control(&prepared, profile)
}

#[cfg(test)]
pub(crate) struct SelectionStats {
    pub(crate) predicted_payload_bits: Option<usize>,
    pub(crate) predicted_payload_bytes: Option<usize>,
    pub(crate) predicted_riff_bytes: Option<usize>,
    pub(crate) losing_single_main_written: bool,
    pub(crate) estimator_fallback: bool,
    pub(crate) candidate_won: bool,
}

#[cfg(test)]
pub(crate) fn encode_profile_exact_for_test(
    width: u32,
    height: u32,
    rgba: &[u8],
    profile: SpatialProfile,
) -> Result<(Vec<u8>, SelectionStats), EncodeError> {
    let prepared = prepare(width, height, rgba)?;
    let plan = SinglePlan::build(&prepared.frequencies);
    let estimates = plan
        .as_ref()
        .ok()
        .map(|plan| (plan.payload_bits(), plan.payload_bytes(), plan.riff_bytes()));
    let (encoded, kind) = encode_prepared_with_plan(&prepared, profile, plan)?;
    Ok((encoded, selection_stats(kind, estimates)))
}

#[cfg(test)]
pub(crate) fn encode_profile_plan_fallback_for_test(
    width: u32,
    height: u32,
    rgba: &[u8],
    profile: SpatialProfile,
) -> Result<(Vec<u8>, SelectionStats), EncodeError> {
    let prepared = prepare(width, height, rgba)?;
    let (encoded, kind) =
        encode_prepared_with_plan(&prepared, profile, Err(EncodeError::output_size_overflow()))?;
    Ok((encoded, selection_stats(kind, None)))
}

#[cfg(test)]
fn selection_stats(
    kind: SelectionKind,
    estimates: Option<(usize, usize, usize)>,
) -> SelectionStats {
    SelectionStats {
        predicted_payload_bits: estimates.map(|values| values.0),
        predicted_payload_bytes: estimates.map(|values| values.1),
        predicted_riff_bytes: estimates.map(|values| values.2),
        losing_single_main_written: !matches!(kind, SelectionKind::Candidate),
        estimator_fallback: matches!(kind, SelectionKind::Fallback),
        candidate_won: matches!(kind, SelectionKind::Candidate),
    }
}

#[cfg(test)]
pub(crate) fn single_estimate_for_test(
    width: u32,
    height: u32,
    rgba: &[u8],
) -> Result<(usize, usize, usize, usize), EncodeError> {
    let prepared = prepare(width, height, rgba)?;
    let plan = SinglePlan::build(&prepared.frequencies)?;
    let mut bits = BitWriter::new();
    write_fast_prefix(&mut bits, &prepared)?;
    plan.write_main_prefix(&mut bits)?;
    write_tokens(&mut bits, &prepared.tokens, plan.tables())?;
    Ok((
        plan.payload_bits(),
        bits.bit_len(),
        plan.payload_bytes(),
        plan.riff_bytes(),
    ))
}
