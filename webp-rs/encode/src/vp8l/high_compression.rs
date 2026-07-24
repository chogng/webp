//! Bounded high-compression transform, LZ77, and entropy portfolio.

use super::entropy_plan::EntropyPlan;
use super::packet_sink::PackedTokenWriter;
use super::predictor_plan::PredictorPlan;
use super::source_analysis::SourceAnalysis;
use super::spatial_plan::{SpatialGrid, SpatialPlan, SpatialProfile, fine_spatial_grid};
use super::token_stream::{CompressedParse, ParseMode, ResidualImage, TokenStream, token_span};
use super::{
    BitWriter, COLOR_TRANSFORM_BLOCK_BITS, ColorTransformPlan, EncodeError, validate_input,
    wrap_vp8l, write_bits, write_color_transform_image, write_compact_entropy_image,
    write_vp8l_header,
};

#[derive(Clone)]
struct TransformCandidate {
    color: Option<ColorTransformPlan>,
    subtract_green: bool,
    predictor: PredictorPlan,
}

struct Candidate {
    transforms: TransformCandidate,
    stream: TokenStream,
    layout: CandidateLayout,
    payload_bits: usize,
}

enum CandidateLayout {
    Single(Box<EntropyPlan>),
    Spatial(Box<SpatialCandidate>),
}

struct SpatialCandidate {
    spatial: SpatialPlan,
    map_stream: TokenStream,
    map_entropy: EntropyPlan,
    groups: Vec<EntropyPlan>,
    encoded_bits: usize,
}

pub(crate) fn encode(width: u32, height: u32, rgba: &[u8]) -> Result<Vec<u8>, EncodeError> {
    validate_input(width, height, rgba)?;
    let width_usize = usize::try_from(width).map_err(|_| EncodeError::input_size_overflow())?;
    let analysis = SourceAnalysis::collect(rgba, width_usize)?;
    let has_alpha = analysis.facts().has_alpha();
    let selected_color =
        super::source_analysis::optimize_color_transform(rgba, analysis.color_transform());
    if let Some(palette) = analysis.into_palette() {
        let (payload, _) =
            super::encode_palette_vp8l_payload(width, height, has_alpha, palette, true)?;
        return wrap_vp8l(payload);
    }
    let mut best: Option<Candidate> = None;
    let mut lazy_portfolio = Vec::new();
    let mut transforms = Vec::new();
    transforms.push(TransformCandidate {
        color: None,
        subtract_green: false,
        predictor: PredictorPlan::None,
    });
    for color in [None, selected_color] {
        if color.is_some() || transforms.len() == 1 {
            transforms.push(TransformCandidate {
                color,
                subtract_green: true,
                predictor: PredictorPlan::None,
            });
            transforms.push(TransformCandidate {
                color,
                subtract_green: true,
                predictor: PredictorPlan::Constant(1),
            });
            transforms.push(TransformCandidate {
                color,
                subtract_green: true,
                predictor: PredictorPlan::adaptive(rgba, width_usize, true, color)?,
            });
        }
    }
    for transforms in transforms {
        let residuals = ResidualImage::collect_with_predictor(
            rgba,
            width_usize,
            transforms.subtract_green,
            transforms.color,
            &transforms.predictor,
        )?;
        let selected_cache = residuals.select_color_cache_bits();
        let parse = residuals.parse_compressed(ParseMode::Greedy)?;
        let mut transform_bits = usize::MAX;
        let mut has_copies = false;
        for color_cache_bits in [0, selected_cache]
            .into_iter()
            .take(if selected_cache == 0 { 1 } else { 2 })
        {
            let candidate = build_candidate(
                width,
                height,
                has_alpha,
                transforms.clone(),
                &residuals,
                &parse,
                color_cache_bits,
            )?;
            transform_bits = transform_bits.min(candidate.payload_bits);
            has_copies |= candidate.stream.statistics().census().copy_tokens() != 0;
            retain_smaller(&mut best, candidate);
        }
        if has_copies {
            retain_lazy_transform(&mut lazy_portfolio, transform_bits, transforms);
        }
    }
    // A finer entropy layout can change which greedy transform is smallest.
    // Refine a bounded portfolio so one layout cannot suppress the strongest
    // match parse of a close transform alternative.
    for (_, lazy_transforms) in lazy_portfolio {
        let lazy_residuals = ResidualImage::collect_with_predictor(
            rgba,
            width_usize,
            lazy_transforms.subtract_green,
            lazy_transforms.color,
            &lazy_transforms.predictor,
        )?;
        let lazy_cache = lazy_residuals.select_color_cache_bits();
        let parse = lazy_residuals.parse_compressed(ParseMode::LazyDeep)?;
        for color_cache_bits in
            [0, lazy_cache]
                .into_iter()
                .take(if lazy_cache == 0 { 1 } else { 2 })
        {
            let candidate = build_candidate(
                width,
                height,
                has_alpha,
                lazy_transforms.clone(),
                &lazy_residuals,
                &parse,
                color_cache_bits,
            )?;
            retain_smaller(&mut best, candidate);
        }
    }
    let best = best.ok_or_else(EncodeError::output_size_overflow)?;
    write_candidate(width, height, has_alpha, best)
}

fn build_candidate(
    width: u32,
    height: u32,
    has_alpha: bool,
    transforms: TransformCandidate,
    residuals: &ResidualImage,
    parse: &CompressedParse,
    color_cache_bits: u8,
) -> Result<Candidate, EncodeError> {
    let stream = TokenStream::collect_compressed_from_parse(residuals, parse, color_cache_bits)?;
    let transform_bits = transform_prefix_bits(width, height, has_alpha, &transforms)?;
    let single = select_entropy(stream.statistics())?;
    let single_bits = single.main_bits(color_cache_bits)?;
    let fine_grid = fine_spatial_grid();
    let compact_grid = SpatialProfile::Compact.grid();
    let (fine_statistics, compact_statistics) =
        stream.spatial_statistics_pair(fine_grid.block_pixels(), compact_grid.block_pixels())?;
    let fine = SpatialCandidate::build(&stream, fine_grid, &fine_statistics)?;
    let compact = SpatialCandidate::build(&stream, compact_grid, &compact_statistics)?;
    let spatial = if fine.encoded_bits < compact.encoded_bits {
        fine
    } else {
        compact
    };
    let (layout, layout_bits) = if spatial.encoded_bits < single_bits {
        let spatial_bits = spatial.encoded_bits;
        (CandidateLayout::Spatial(Box::new(spatial)), spatial_bits)
    } else {
        (CandidateLayout::Single(Box::new(single)), single_bits)
    };
    let payload_bits = transform_bits
        .checked_add(layout_bits)
        .ok_or_else(EncodeError::output_size_overflow)?;
    Ok(Candidate {
        transforms,
        stream,
        layout,
        payload_bits,
    })
}

fn retain_smaller(best: &mut Option<Candidate>, candidate: Candidate) {
    if best
        .as_ref()
        .is_none_or(|current| candidate.payload_bits < current.payload_bits)
    {
        *best = Some(candidate);
    }
}

fn retain_lazy_transform(
    portfolio: &mut Vec<(usize, TransformCandidate)>,
    payload_bits: usize,
    transforms: TransformCandidate,
) {
    let position = portfolio
        .iter()
        .position(|(current, _)| payload_bits < *current)
        .unwrap_or(portfolio.len());
    portfolio.insert(position, (payload_bits, transforms));
    portfolio.truncate(2);
}

fn select_entropy(
    statistics: &super::token_stream::TokenStatistics,
) -> Result<EntropyPlan, EncodeError> {
    EntropyPlan::build_compact_for_stream(statistics)
}

impl SpatialCandidate {
    fn build(
        stream: &TokenStream,
        grid: SpatialGrid,
        statistics: &super::token_stream::SpatialBlockStatistics,
    ) -> Result<Self, EncodeError> {
        let spatial = SpatialPlan::build_for_grid_with_statistics(stream, grid, statistics)?;
        let map_stream = build_group_map_stream(&spatial)?;
        let map_entropy = select_entropy(map_stream.statistics())?;
        let mut groups = Vec::new();
        groups
            .try_reserve_exact(spatial.frequencies().len())
            .map_err(|_| EncodeError::allocation_failed())?;
        for frequencies in spatial.frequencies() {
            groups.push(EntropyPlan::build_compact(frequencies)?);
        }
        let cache_bits = 1 + usize::from(stream.color_cache_bits() != 0) * 4;
        let mut encoded_bits = cache_bits
            .checked_add(4)
            .and_then(|bits| bits.checked_add(1))
            .and_then(|bits| bits.checked_add(map_entropy.encoded_bits().ok()?))
            .ok_or_else(EncodeError::output_size_overflow)?;
        for group in &groups {
            encoded_bits = encoded_bits
                .checked_add(group.encoded_bits()?)
                .ok_or_else(EncodeError::output_size_overflow)?;
        }
        Ok(Self {
            spatial,
            map_stream,
            map_entropy,
            groups,
            encoded_bits,
        })
    }
}

fn transform_prefix_bits(
    width: u32,
    height: u32,
    has_alpha: bool,
    transforms: &TransformCandidate,
) -> Result<usize, EncodeError> {
    let mut bits = BitWriter::new();
    write_transform_prefix(&mut bits, width, height, has_alpha, transforms)?;
    Ok(bits.bit_len())
}

fn write_candidate(
    width: u32,
    height: u32,
    has_alpha: bool,
    candidate: Candidate,
) -> Result<Vec<u8>, EncodeError> {
    let mut bits = BitWriter::new();
    write_transform_prefix(&mut bits, width, height, has_alpha, &candidate.transforms)?;
    let packed = match &candidate.layout {
        CandidateLayout::Single(entropy) => {
            entropy.write_main_prefix(&mut bits, candidate.stream.color_cache_bits())?;
            let mut packed = PackedTokenWriter::from_prefix(bits, entropy.token_bits())?;
            for &token in candidate.stream.tokens() {
                packed.write_token(token, entropy.tables())?;
            }
            packed
        }
        CandidateLayout::Spatial(spatial) => {
            write_spatial_tokens(bits, &candidate.stream, spatial)?
        }
    };
    if packed.bit_len() != candidate.payload_bits {
        return Err(EncodeError::output_size_overflow());
    }
    wrap_vp8l(packed.finish()?)
}

fn write_spatial_tokens(
    mut bits: BitWriter,
    stream: &TokenStream,
    candidate: &SpatialCandidate,
) -> Result<PackedTokenWriter, EncodeError> {
    let color_cache_bits = stream.color_cache_bits();
    write_bits(&mut bits, u32::from(color_cache_bits != 0), 1)?;
    if color_cache_bits != 0 {
        write_bits(&mut bits, u32::from(color_cache_bits), 4)?;
    }
    write_bits(&mut bits, 1, 1)?;
    write_bits(&mut bits, u32::from(candidate.spatial.wire_block_bits()), 3)?;
    write_bits(&mut bits, 0, 1)?;
    candidate.map_entropy.write_tables(&mut bits)?;
    let mut map_sink = PackedTokenWriter::from_prefix(bits, candidate.map_entropy.token_bits())?;
    for &token in candidate.map_stream.tokens() {
        map_sink.write_token(token, candidate.map_entropy.tables())?;
    }
    let mut bits = map_sink.into_prefix()?;
    for group in &candidate.groups {
        group.write_tables(&mut bits)?;
    }
    let token_bits = candidate.groups.iter().try_fold(0_usize, |total, group| {
        total.checked_add(group.token_bits())
    });
    let mut packed = PackedTokenWriter::from_prefix(
        bits,
        token_bits.ok_or_else(EncodeError::output_size_overflow)?,
    )?;
    let mut pixel = 0_usize;
    for &token in stream.tokens() {
        let group = candidate.spatial.group_for_pixel(pixel);
        let entropy = candidate
            .groups
            .get(group)
            .ok_or_else(EncodeError::output_size_overflow)?;
        packed.write_token(token, entropy.tables())?;
        pixel = pixel
            .checked_add(token_span(token))
            .ok_or_else(EncodeError::output_size_overflow)?;
    }
    Ok(packed)
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

fn write_transform_prefix(
    bits: &mut BitWriter,
    width: u32,
    height: u32,
    has_alpha: bool,
    transforms: &TransformCandidate,
) -> Result<(), EncodeError> {
    write_vp8l_header(bits, width, height, has_alpha)?;
    if let Some(color) = transforms.color {
        write_bits(bits, 1, 1)?;
        write_bits(bits, 1, 2)?;
        write_bits(bits, u32::from(COLOR_TRANSFORM_BLOCK_BITS - 2), 3)?;
        write_color_transform_image(bits, width, height, color)?;
    }
    if transforms.subtract_green {
        write_bits(bits, 1, 1)?;
        write_bits(bits, 2, 2)?;
    }
    if transforms.predictor.is_present() {
        write_bits(bits, 1, 1)?;
        write_bits(bits, 0, 2)?;
        write_bits(bits, u32::from(transforms.predictor.block_bits() - 2), 3)?;
        write_predictor_plan(bits, width, height, &transforms.predictor)?;
    }
    write_bits(bits, 0, 1)
}

fn write_predictor_plan(
    writer: &mut BitWriter,
    width: u32,
    height: u32,
    plan: &PredictorPlan,
) -> Result<(), EncodeError> {
    let block_size = 1_u32 << plan.block_bits();
    let mode_width = width.div_ceil(block_size);
    let mode_height = height.div_ceil(block_size);
    let mode_pixels =
        build_predictor_mode_pixels(width, mode_width, mode_height, block_size, plan)?;
    write_compact_entropy_image(
        writer,
        &mode_pixels,
        usize::try_from(mode_width).map_err(|_| EncodeError::output_size_overflow())?,
    )
}

fn build_predictor_mode_pixels(
    width: u32,
    mode_width: u32,
    mode_height: u32,
    block_size: u32,
    plan: &PredictorPlan,
) -> Result<Vec<u8>, EncodeError> {
    let pixel_count = usize::try_from(mode_width)
        .ok()
        .and_then(|mode_width| {
            usize::try_from(mode_height)
                .ok()
                .and_then(|mode_height| mode_width.checked_mul(mode_height))
        })
        .ok_or_else(EncodeError::output_size_overflow)?;
    let mut pixels = Vec::new();
    pixels
        .try_reserve_exact(
            pixel_count
                .checked_mul(4)
                .ok_or_else(EncodeError::output_size_overflow)?,
        )
        .map_err(|_| EncodeError::allocation_failed())?;
    for y in 0..mode_height {
        for x in 0..mode_width {
            let source_x =
                usize::try_from(x * block_size).map_err(|_| EncodeError::output_size_overflow())?;
            let source_y =
                usize::try_from(y * block_size).map_err(|_| EncodeError::output_size_overflow())?;
            let source_width =
                usize::try_from(width).map_err(|_| EncodeError::output_size_overflow())?;
            let mode = plan
                .mode_at(source_y * source_width + source_x, source_width)
                .ok_or_else(EncodeError::output_size_overflow)?;
            pixels.extend_from_slice(&[0, mode, 0, u8::MAX]);
        }
    }
    Ok(pixels)
}

#[cfg(test)]
#[path = "high_compression_tests.rs"]
mod tests;
