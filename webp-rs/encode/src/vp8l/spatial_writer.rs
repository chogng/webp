//! Standard VP8L spatial planning, exact selection, and packet serialization.

use super::entropy_plan::{EntropyPlan, riff_bytes};
use super::packet_sink::PackedTokenWriter;
use super::portfolio_policy::{PortfolioCost, choose};
use super::spatial_plan::{SpatialPlan, SpatialProfile};
use super::token_stream::{TokenStream, token_span};
use super::{BitWriter, EncodeError, validate_input, wrap_vp8l, write_bits, write_vp8l_header};

const FAST_PREFIX_BITS: usize = 44;
const SPATIAL_CONFIG_BITS: usize = 5;
const NESTED_MAP_CONFIG_BITS: usize = 1;

struct Prepared {
    width_u32: u32,
    height_u32: u32,
    has_alpha: bool,
    stream: TokenStream,
}

struct SingleCandidate {
    entropy: EntropyPlan,
    payload_bits: usize,
}

impl SingleCandidate {
    fn build(stream: &TokenStream) -> Result<Self, EncodeError> {
        let entropy = EntropyPlan::build_for_stream(stream.statistics())?;
        let payload_bits = FAST_PREFIX_BITS
            .checked_add(entropy.main_bits(stream.color_cache_bits())?)
            .ok_or_else(EncodeError::output_size_overflow)?;
        riff_bytes(payload_bits)?;
        Ok(Self {
            entropy,
            payload_bits,
        })
    }

    fn cost(&self) -> PortfolioCost {
        PortfolioCost {
            exact_bits: self.payload_bits,
            secondary_lookups: self.entropy.secondary_lookups(),
            group_runs: 0,
            table_map_bytes: 5 * 2048,
        }
    }
}

struct SpatialCandidate {
    profile: SpatialProfile,
    spatial: SpatialPlan,
    map_stream: TokenStream,
    map_entropy: EntropyPlan,
    groups: Vec<EntropyPlan>,
    payload_bits: usize,
}

impl SpatialCandidate {
    fn build(stream: &TokenStream, profile: SpatialProfile) -> Result<Self, EncodeError> {
        let spatial = SpatialPlan::build(stream, profile)?;
        let map_stream = build_group_map_stream(&spatial)?;
        let map_entropy = EntropyPlan::build_for_stream(map_stream.statistics())?;
        let mut groups = Vec::new();
        groups
            .try_reserve_exact(spatial.frequencies().len())
            .map_err(|_| EncodeError::allocation_failed())?;
        for frequencies in spatial.frequencies() {
            groups.push(EntropyPlan::build(frequencies)?);
        }

        let mut payload_bits = FAST_PREFIX_BITS
            .checked_add(SPATIAL_CONFIG_BITS)
            .and_then(|bits| bits.checked_add(NESTED_MAP_CONFIG_BITS))
            .and_then(|bits| bits.checked_add(map_entropy.encoded_bits().ok()?))
            .ok_or_else(EncodeError::output_size_overflow)?;
        for group in &groups {
            payload_bits = payload_bits
                .checked_add(group.encoded_bits()?)
                .ok_or_else(EncodeError::output_size_overflow)?;
        }
        riff_bytes(payload_bits)?;
        Ok(Self {
            profile,
            spatial,
            map_stream,
            map_entropy,
            groups,
            payload_bits,
        })
    }

    fn cost(&self) -> Result<PortfolioCost, EncodeError> {
        let secondary_lookups =
            self.groups
                .iter()
                .try_fold(self.map_entropy.secondary_lookups(), |total, group| {
                    total
                        .checked_add(group.secondary_lookups())
                        .ok_or_else(EncodeError::output_size_overflow)
                })?;
        Ok(PortfolioCost {
            exact_bits: self.payload_bits,
            secondary_lookups,
            group_runs: self.spatial.decode_group_runs()?,
            table_map_bytes: self.spatial.table_map_bytes()?,
        })
    }
}

struct ProfilePlan {
    single: SingleCandidate,
    spatial: SpatialCandidate,
}

impl ProfilePlan {
    fn build(stream: &TokenStream, profile: SpatialProfile) -> Result<Self, EncodeError> {
        Ok(Self {
            single: SingleCandidate::build(stream)?,
            spatial: SpatialCandidate::build(stream, profile)?,
        })
    }

    fn spatial(&self, profile: SpatialProfile) -> Result<&SpatialCandidate, EncodeError> {
        if self.spatial.profile != profile {
            return Err(EncodeError::output_size_overflow());
        }
        Ok(&self.spatial)
    }

    fn selected(&self, profile: SpatialProfile) -> Result<usize, EncodeError> {
        let costs = [self.single.cost(), self.spatial(profile)?.cost()?];
        choose(profile, &costs)
            .map(|index| match profile {
                SpatialProfile::Compact => index,
                SpatialProfile::LowLatency => index * 2,
            })
            .ok_or_else(EncodeError::output_size_overflow)
    }
}

pub fn encode_profile(
    width: u32,
    height: u32,
    rgba: &[u8],
    profile: SpatialProfile,
) -> Result<Vec<u8>, EncodeError> {
    let prepared = prepare_spatial(width, height, rgba)?;
    encode_prepared(&prepared, profile).map(|(encoded, _)| encoded)
}

fn encode_profile_control(
    prepared: &Prepared,
    profile: SpatialProfile,
) -> Result<Vec<u8>, EncodeError> {
    let single_plan = SingleCandidate::build(&prepared.stream)?;
    let spatial_plan = SpatialCandidate::build(&prepared.stream, profile)?;
    let single = encode_single_with_plan(prepared, &single_plan)?;
    let candidate = encode_spatial_with_plan(prepared, profile, &spatial_plan)?;
    Ok(if candidate.len() < single.len() {
        candidate
    } else {
        single
    })
}

#[derive(Clone, Copy)]
enum SelectionKind {
    Spatial,
    Single,
    Fallback,
}

fn encode_prepared(
    prepared: &Prepared,
    profile: SpatialProfile,
) -> Result<(Vec<u8>, SelectionKind), EncodeError> {
    encode_prepared_with_plan(
        prepared,
        profile,
        ProfilePlan::build(&prepared.stream, profile),
    )
}

fn encode_prepared_with_plan(
    prepared: &Prepared,
    profile: SpatialProfile,
    plan: Result<ProfilePlan, EncodeError>,
) -> Result<(Vec<u8>, SelectionKind), EncodeError> {
    let (plan, selected) = match plan.and_then(|plan| {
        let selected = plan.selected(profile)?;
        Ok((plan, selected))
    }) {
        Ok(selected) => selected,
        Err(_) => {
            return encode_profile_control(prepared, profile)
                .map(|encoded| (encoded, SelectionKind::Fallback));
        }
    };
    if selected == 0 {
        encode_single_with_plan(prepared, &plan.single)
            .map(|encoded| (encoded, SelectionKind::Single))
    } else {
        let selected_profile = match selected {
            1 => SpatialProfile::Compact,
            2 => SpatialProfile::LowLatency,
            _ => return Err(EncodeError::output_size_overflow()),
        };
        let candidate = plan.spatial(selected_profile)?;
        encode_spatial_with_plan(prepared, candidate.profile, candidate)
            .map(|encoded| (encoded, SelectionKind::Spatial))
    }
}

#[cfg(test)]
pub(crate) const fn candidate_wins(candidate_bytes: usize, single_bytes: usize) -> bool {
    candidate_bytes < single_bytes
}

#[cfg(test)]
fn prepare(width: u32, height: u32, rgba: &[u8]) -> Result<Prepared, EncodeError> {
    validate_input(width, height, rgba)?;
    let width_usize = usize::try_from(width).map_err(|_| EncodeError::input_size_overflow())?;
    let stream = TokenStream::collect(rgba, width_usize, true, false, 0)?;
    Ok(Prepared {
        width_u32: width,
        height_u32: height,
        has_alpha: rgba.chunks_exact(4).any(|pixel| pixel[3] != u8::MAX),
        stream,
    })
}

fn prepare_spatial(width: u32, height: u32, rgba: &[u8]) -> Result<Prepared, EncodeError> {
    validate_input(width, height, rgba)?;
    let width_usize = usize::try_from(width).map_err(|_| EncodeError::input_size_overflow())?;
    let stream = TokenStream::collect(rgba, width_usize, true, false, 0)?;
    Ok(Prepared {
        width_u32: width,
        height_u32: height,
        has_alpha: rgba.chunks_exact(4).any(|pixel| pixel[3] != u8::MAX),
        stream,
    })
}

#[cfg(test)]
fn encode_single(prepared: &Prepared) -> Result<Vec<u8>, EncodeError> {
    let plan = SingleCandidate::build(&prepared.stream)?;
    encode_single_with_plan(prepared, &plan)
}

fn encode_single_with_plan(
    prepared: &Prepared,
    plan: &SingleCandidate,
) -> Result<Vec<u8>, EncodeError> {
    let (payload, written_bits) = write_single_payload(prepared, plan)?;
    if written_bits != plan.payload_bits {
        return Err(EncodeError::output_size_overflow());
    }
    wrap_vp8l(payload)
}

fn write_single_payload(
    prepared: &Prepared,
    plan: &SingleCandidate,
) -> Result<(Vec<u8>, usize), EncodeError> {
    let mut bits = BitWriter::new();
    write_fast_prefix(&mut bits, prepared)?;
    plan.entropy
        .write_main_prefix(&mut bits, prepared.stream.color_cache_bits())?;
    let mut packed = PackedTokenWriter::from_prefix(bits, plan.entropy.token_bits())?;
    for &token in prepared.stream.tokens() {
        packed.write_token(token, plan.entropy.tables())?;
    }
    let written_bits = packed.bit_len();
    Ok((packed.finish()?, written_bits))
}

#[cfg(test)]
fn encode_spatial(prepared: &Prepared, profile: SpatialProfile) -> Result<Vec<u8>, EncodeError> {
    let plan = SpatialCandidate::build(&prepared.stream, profile)?;
    encode_spatial_with_plan(prepared, profile, &plan)
}

fn encode_spatial_with_plan(
    prepared: &Prepared,
    profile: SpatialProfile,
    plan: &SpatialCandidate,
) -> Result<Vec<u8>, EncodeError> {
    let (payload, written_bits) = write_spatial_payload(prepared, profile, plan)?;
    if written_bits != plan.payload_bits {
        return Err(EncodeError::output_size_overflow());
    }
    wrap_vp8l(payload)
}

fn write_spatial_payload(
    prepared: &Prepared,
    profile: SpatialProfile,
    plan: &SpatialCandidate,
) -> Result<(Vec<u8>, usize), EncodeError> {
    let mut bits = BitWriter::new();
    write_fast_prefix(&mut bits, prepared)?;
    write_bits(&mut bits, 0, 1)?;
    write_bits(&mut bits, 1, 1)?;
    write_bits(&mut bits, u32::from(profile.wire_block_bits()), 3)?;

    write_bits(&mut bits, 0, 1)?;
    plan.map_entropy.write_tables(&mut bits)?;
    let mut map_sink = PackedTokenWriter::from_prefix(bits, plan.map_entropy.token_bits())?;
    for &token in plan.map_stream.tokens() {
        map_sink.write_token(token, plan.map_entropy.tables())?;
    }
    let mut bits = map_sink.into_prefix()?;

    for group in &plan.groups {
        group.write_tables(&mut bits)?;
    }
    let token_bits = plan.groups.iter().try_fold(0_usize, |total, group| {
        total.checked_add(group.token_bits())
    });
    let mut packed = PackedTokenWriter::from_prefix(
        bits,
        token_bits.ok_or_else(EncodeError::output_size_overflow)?,
    )?;
    let mut pixel = 0_usize;
    for &token in prepared.stream.tokens() {
        let group = plan.spatial.group_for_pixel(pixel);
        let entropy = plan
            .groups
            .get(group)
            .ok_or_else(EncodeError::output_size_overflow)?;
        packed.write_token(token, entropy.tables())?;
        pixel = pixel
            .checked_add(token_span(token))
            .ok_or_else(EncodeError::output_size_overflow)?;
    }
    let written_bits = packed.bit_len();
    Ok((packed.finish()?, written_bits))
}

fn write_fast_prefix(bits: &mut BitWriter, prepared: &Prepared) -> Result<(), EncodeError> {
    write_vp8l_header(
        bits,
        prepared.width_u32,
        prepared.height_u32,
        prepared.has_alpha,
    )?;
    write_bits(bits, 1, 1)?;
    write_bits(bits, 2, 2)?;
    write_bits(bits, 0, 1)
}

fn build_group_map_stream(plan: &SpatialPlan) -> Result<TokenStream, EncodeError> {
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
    TokenStream::collect(&rgba, plan.map_width(), false, false, 0)
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
    let prepared = prepare_spatial(width, height, rgba)?;
    encode_spatial(&prepared, profile)
}

#[cfg(test)]
pub(crate) fn encode_profile_control_for_test(
    width: u32,
    height: u32,
    rgba: &[u8],
    profile: SpatialProfile,
) -> Result<Vec<u8>, EncodeError> {
    let prepared = prepare_spatial(width, height, rgba)?;
    encode_profile_control(&prepared, profile)
}

#[cfg(test)]
pub(crate) struct SelectionStats {
    pub(crate) predicted_payload_bits: Option<usize>,
    pub(crate) predicted_payload_bytes: Option<usize>,
    pub(crate) predicted_riff_bytes: Option<usize>,
    pub(crate) predicted_candidate_payload_bits: Option<usize>,
    pub(crate) predicted_candidate_riff_bytes: Option<usize>,
    pub(crate) losing_single_main_written: bool,
    pub(crate) losing_candidate_main_written: bool,
    pub(crate) estimator_fallback: bool,
    pub(crate) candidate_won: bool,
    pub(crate) selected_profile: Option<SpatialProfile>,
    pub(crate) portfolio_costs: Option<[PortfolioCost; 3]>,
    pub(crate) selected_index: Option<usize>,
}

#[cfg(test)]
pub(crate) fn encode_profile_exact_for_test(
    width: u32,
    height: u32,
    rgba: &[u8],
    profile: SpatialProfile,
) -> Result<(Vec<u8>, SelectionStats), EncodeError> {
    let prepared = prepare_spatial(width, height, rgba)?;
    let plan = ProfilePlan::build(&prepared.stream, profile);
    let costs = plan.as_ref().ok().and_then(|plan| {
        let requested = plan.spatial(profile).ok()?.cost().ok()?;
        let other_profile = match profile {
            SpatialProfile::Compact => SpatialProfile::LowLatency,
            SpatialProfile::LowLatency => SpatialProfile::Compact,
        };
        let other_prepared = prepare_spatial(width, height, rgba).ok()?;
        let other = SpatialCandidate::build(&other_prepared.stream, other_profile)
            .ok()?
            .cost()
            .ok()?;
        Some(match profile {
            SpatialProfile::Compact => [plan.single.cost(), requested, other],
            SpatialProfile::LowLatency => [plan.single.cost(), other, requested],
        })
    });
    let selected_index = plan
        .as_ref()
        .ok()
        .and_then(|plan| plan.selected(profile).ok());
    let estimates = plan.as_ref().ok().and_then(|plan| {
        let candidate = plan.spatial(profile).ok()?;
        Some((
            plan.single.payload_bits,
            super::entropy_plan::payload_bytes(plan.single.payload_bits).ok(),
            riff_bytes(plan.single.payload_bits).ok()?,
            candidate.payload_bits,
            riff_bytes(candidate.payload_bits).ok()?,
        ))
    });
    let (encoded, kind) = encode_prepared_with_plan(&prepared, profile, plan)?;
    Ok((
        encoded,
        selection_stats(kind, estimates, costs, selected_index),
    ))
}

#[cfg(test)]
pub(crate) fn encode_profile_plan_fallback_for_test(
    width: u32,
    height: u32,
    rgba: &[u8],
    profile: SpatialProfile,
) -> Result<(Vec<u8>, SelectionStats), EncodeError> {
    let prepared = prepare_spatial(width, height, rgba)?;
    let (encoded, kind) =
        encode_prepared_with_plan(&prepared, profile, Err(EncodeError::output_size_overflow()))?;
    Ok((encoded, selection_stats(kind, None, None, None)))
}

#[cfg(test)]
type Estimates = (usize, Option<usize>, usize, usize, usize);

#[cfg(test)]
fn selection_stats(
    kind: SelectionKind,
    estimates: Option<Estimates>,
    portfolio_costs: Option<[PortfolioCost; 3]>,
    selected_index: Option<usize>,
) -> SelectionStats {
    SelectionStats {
        predicted_payload_bits: estimates.map(|values| values.0),
        predicted_payload_bytes: estimates.and_then(|values| values.1),
        predicted_riff_bytes: estimates.map(|values| values.2),
        predicted_candidate_payload_bits: estimates.map(|values| values.3),
        predicted_candidate_riff_bytes: estimates.map(|values| values.4),
        losing_single_main_written: matches!(kind, SelectionKind::Fallback),
        losing_candidate_main_written: matches!(kind, SelectionKind::Fallback),
        estimator_fallback: matches!(kind, SelectionKind::Fallback),
        candidate_won: matches!(kind, SelectionKind::Spatial),
        selected_profile: match (kind, selected_index) {
            (SelectionKind::Spatial, Some(1)) => Some(SpatialProfile::Compact),
            (SelectionKind::Spatial, Some(2)) => Some(SpatialProfile::LowLatency),
            (SelectionKind::Spatial, _)
            | (SelectionKind::Single, _)
            | (SelectionKind::Fallback, _) => None,
        },
        portfolio_costs,
        selected_index,
    }
}

#[cfg(test)]
pub(crate) fn single_estimate_for_test(
    width: u32,
    height: u32,
    rgba: &[u8],
) -> Result<(usize, usize, usize, usize), EncodeError> {
    let prepared = prepare(width, height, rgba)?;
    let plan = SingleCandidate::build(&prepared.stream)?;
    let (_, written_bits) = write_single_payload(&prepared, &plan)?;
    Ok((
        plan.payload_bits,
        written_bits,
        super::entropy_plan::payload_bytes(plan.payload_bits)?,
        riff_bytes(plan.payload_bits)?,
    ))
}

#[cfg(test)]
pub(crate) fn candidate_estimate_for_test(
    width: u32,
    height: u32,
    rgba: &[u8],
    profile: SpatialProfile,
) -> Result<(usize, usize, usize, usize), EncodeError> {
    let prepared = prepare_spatial(width, height, rgba)?;
    let plan = SpatialCandidate::build(&prepared.stream, profile)?;
    let (_, written_bits) = write_spatial_payload(&prepared, profile, &plan)?;
    Ok((
        plan.payload_bits,
        written_bits,
        super::entropy_plan::payload_bytes(plan.payload_bits)?,
        riff_bytes(plan.payload_bits)?,
    ))
}
