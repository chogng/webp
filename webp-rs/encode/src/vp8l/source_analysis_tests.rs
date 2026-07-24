use super::*;

#[test]
fn analysis_owns_recomputable_identity_alpha_palette_and_geometry() {
    let rgba = [
        1, 2, 3, 255, //
        4, 5, 6, 0, //
        1, 2, 3, 255, //
        7, 8, 9, 17,
    ];
    let analysis = SourceAnalysis::collect(&rgba, 2).expect("analyze source");
    let facts = analysis.facts();

    assert_eq!((facts.width(), facts.height(), facts.pixels()), (2, 2, 4));
    assert_eq!(facts.identity().rgba_bytes(), rgba.len());
    assert_eq!(facts.identity().fnv1a64(), fnv1a(&rgba));
    assert_eq!(facts.non_opaque_pixels(), 2);
    assert_eq!(facts.transparent_pixels(), 1);
    assert_eq!(facts.palette_colors(), Some(3));
    assert!(facts.has_alpha());

    let palette = analysis.into_palette().expect("bounded palette plan");
    assert_eq!(palette.entries().len(), 3);
    assert_eq!(palette.indexed_width(), 1);
    assert_eq!(palette.indexed_rgba().len(), 8);
}

#[test]
fn fused_color_scores_match_the_established_candidate_selection() {
    let mut rgba = Vec::new();
    for y in 0_u16..32 {
        for x in 0_u16..32 {
            let green = x.wrapping_mul(7).wrapping_add(y.wrapping_mul(11)) as u8;
            rgba.extend_from_slice(&[
                green.wrapping_add((x & 3) as u8),
                green,
                green.wrapping_add((y & 3) as u8),
                u8::MAX,
            ]);
        }
    }
    let selected = SourceAnalysis::collect(&rgba, 32)
        .expect("analyze correlated source")
        .color_transform();
    assert_eq!(selected, established_color_transform_selection(&rgba));
}

#[test]
fn full_byte_palette_is_remapped_into_unpacked_indices() {
    let mut rgba = Vec::new();
    for index in 0..256_u16 {
        rgba.extend_from_slice(&[
            index as u8,
            index.wrapping_mul(29) as u8,
            index.wrapping_mul(71) as u8,
            u8::MAX,
        ]);
    }
    let analysis = SourceAnalysis::collect(&rgba, 256).expect("analyze byte palette");
    assert_eq!(analysis.facts().palette_colors(), Some(256));
    let palette = analysis.into_palette().expect("build byte palette");
    assert_eq!(palette.entries().len(), 256);
    assert_eq!(palette.indexed_width(), 256);
    assert_eq!(palette.indexed_rgba().len(), rgba.len());
}

fn established_color_transform_selection(rgba: &[u8]) -> Option<ColorTransformPlan> {
    if rgba.len() / 4 < MIN_COLOR_TRANSFORM_PIXELS {
        return None;
    }
    let baseline = established_score(rgba, None);
    let mut selected = None;
    let mut best = baseline;
    for candidate in COLOR_TRANSFORM_CANDIDATES {
        let score = established_score(rgba, Some(candidate));
        if score < best {
            best = score;
            selected = Some(candidate);
        }
    }
    (best.saturating_mul(4) <= baseline.saturating_mul(3)).then_some(selected?)
}

fn established_score(rgba: &[u8], plan: Option<ColorTransformPlan>) -> u64 {
    rgba.chunks_exact(4)
        .map(|pixel| {
            let transformed = plan.map_or([pixel[0], pixel[1], pixel[2], pixel[3]], |plan| {
                forward_color_pixel(pixel, plan)
            });
            color_score(transformed[0], transformed[2])
        })
        .sum()
}

fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(FNV_OFFSET_BASIS, |hash, &byte| {
        (hash ^ u64::from(byte)).wrapping_mul(FNV_PRIME)
    })
}
