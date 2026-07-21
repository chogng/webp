use super::*;
use crate::FilterHeader;
use crate::SegmentHeader;

#[test]
fn derives_strengths_with_deltas_sharpness_and_segments() {
    let filter = FilterHeader {
        simple: false,
        level: 17,
        sharpness: 4,
        use_deltas: true,
        ref_deltas: [2, 0, 0, 0],
        mode_deltas: [-1, 0, 0, 0],
    };
    let disabled_segments = SegmentHeader {
        enabled: false,
        update_map: false,
        absolute_delta: true,
        quantizer: [0; 4],
        filter_strength: [0; 4],
        probabilities: [255; 3],
    };
    let strengths = derive_loop_filter_strengths(&filter, &disabled_segments);
    assert_eq!(
        strengths[0][0],
        LoopFilterStrength {
            level: 19,
            inner_limit: 5,
            edge_limit: 43,
            hev_threshold: 1,
        }
    );
    assert_eq!(
        strengths[0][1],
        LoopFilterStrength {
            level: 18,
            inner_limit: 5,
            edge_limit: 41,
            hev_threshold: 1,
        }
    );
    assert!(strengths[0][1].filters_inner(true, true));
    assert!(strengths[0][0].filters_inner(false, false));
    assert!(!strengths[0][0].filters_inner(false, true));

    let segments = SegmentHeader {
        enabled: true,
        update_map: true,
        absolute_delta: false,
        quantizer: [0; 4],
        filter_strength: [-30, 50, 0, 80],
        probabilities: [0; 3],
    };
    let segmented = derive_loop_filter_strengths(&filter, &segments);
    assert_eq!(segmented[0], [LoopFilterStrength::default(); 2]);
    assert_eq!(segmented[1][0].level, 63);
    assert_eq!(segmented[1][0].inner_limit, 5);
    assert_eq!(segmented[1][0].edge_limit, 131);
    assert_eq!(segmented[1][0].hev_threshold, 2);
    assert_eq!(segmented[3][1].level, 63);
}

#[test]
fn zero_base_filter_level_disables_deltas_for_the_whole_frame() {
    let filter = FilterHeader {
        simple: false,
        level: 0,
        sharpness: 0,
        use_deltas: true,
        ref_deltas: [2, 0, -2, -2],
        mode_deltas: [4, -2, 2, 4],
    };
    let segments = SegmentHeader {
        enabled: true,
        update_map: true,
        absolute_delta: true,
        quantizer: [0; 4],
        filter_strength: [63; 4],
        probabilities: [0; 3],
    };
    assert_eq!(
        derive_loop_filter_strengths(&filter, &segments),
        [[LoopFilterStrength::default(); 2]; 4]
    );
}

#[test]
fn scalar_filters_match_vp8_two_four_and_six_tap_rules() {
    let mut simple = [100, 100, 100, 110, 110, 110];
    assert!(filter_simple_edge(&mut simple, 3, 1, 25));
    assert_eq!(simple, [100, 100, 102, 107, 110, 110]);

    let hev_strength = LoopFilterStrength {
        level: 20,
        inner_limit: 100,
        edge_limit: 50,
        hev_threshold: 0,
    };
    let mut high_variance = [100, 100, 100, 100, 110, 140, 140, 140];
    assert!(filter_normal_edge(
        &mut high_variance,
        4,
        1,
        hev_strength,
        true
    ));
    assert_eq!(high_variance, [100, 100, 100, 99, 111, 140, 140, 140]);

    let smooth_strength = LoopFilterStrength {
        level: 20,
        inner_limit: 10,
        edge_limit: 100,
        hev_threshold: 20,
    };
    let mut macroblock = [100, 100, 100, 100, 110, 110, 110, 110];
    assert!(filter_normal_edge(
        &mut macroblock,
        4,
        1,
        smooth_strength,
        true
    ));
    assert_eq!(macroblock, [100, 101, 103, 104, 106, 107, 109, 110]);

    let mut inner = [100, 100, 100, 100, 110, 110, 110, 110];
    assert!(filter_normal_edge(&mut inner, 4, 1, smooth_strength, false));
    assert_eq!(inner, [100, 100, 102, 104, 106, 108, 110, 110]);
}

#[test]
fn row_filter_applies_luma_internal_edges_only_when_requested() {
    let strength = LoopFilterStrength {
        level: 10,
        inner_limit: 10,
        edge_limit: 25,
        hev_threshold: 0,
    };
    let mut y = vec![0; 16 * 16];
    for row in y.chunks_exact_mut(16) {
        row[..4].fill(100);
        row[4..].fill(110);
    }
    let mut u = vec![128; 8 * 8];
    let mut v = vec![128; 8 * 8];
    filter_macroblock(MacroblockFilter {
        y: &mut y,
        u: &mut u,
        v: &mut v,
        y_stride: 16,
        uv_stride: 8,
        macroblock_x: 0,
        macroblock_y: 0,
        simple: true,
        strength,
        filters_inner: true,
    });
    for row in y.chunks_exact(16) {
        assert_eq!(&row[2..6], &[100, 102, 107, 110]);
    }

    let mut untouched = vec![0; 16 * 16];
    for row in untouched.chunks_exact_mut(16) {
        row[..4].fill(100);
        row[4..].fill(110);
    }
    filter_macroblock(MacroblockFilter {
        y: &mut untouched,
        u: &mut u,
        v: &mut v,
        y_stride: 16,
        uv_stride: 8,
        macroblock_x: 0,
        macroblock_y: 0,
        simple: true,
        strength,
        filters_inner: false,
    });
    assert!(
        untouched
            .iter()
            .all(|&sample| sample == 100 || sample == 110)
    );
}

#[test]
fn scalar_filters_skip_out_of_bounds_and_sharp_edges() {
    let strength = LoopFilterStrength {
        level: 10,
        inner_limit: 5,
        edge_limit: 10,
        hev_threshold: 0,
    };
    let mut short = [100_u8; 4];
    assert!(!filter_simple_edge(&mut short, 1, 1, 10));
    assert!(!filter_normal_edge(&mut short, 2, 1, strength, true));

    let mut sharp = [0, 0, 0, 0, 255, 255, 255, 255];
    assert!(!filter_normal_edge(&mut sharp, 4, 1, strength, true));
    assert_eq!(sharp, [0, 0, 0, 0, 255, 255, 255, 255]);
}
