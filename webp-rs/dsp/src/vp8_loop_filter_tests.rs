//! Tests for VP8 loop-filter edge kernels.

use super::*;

#[test]
fn scalar_filters_match_two_four_and_six_tap_rules() {
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
        true,
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
        true,
    ));
    assert_eq!(macroblock, [100, 101, 103, 104, 106, 107, 109, 110]);

    let mut inner = [100, 100, 100, 100, 110, 110, 110, 110];
    assert!(filter_normal_edge(&mut inner, 4, 1, smooth_strength, false));
    assert_eq!(inner, [100, 100, 102, 104, 106, 108, 110, 110]);
}

#[test]
fn filters_reject_invalid_or_sharp_edges_without_mutation() {
    let strength = LoopFilterStrength {
        level: 10,
        inner_limit: 5,
        edge_limit: 10,
        hev_threshold: 0,
    };
    let mut short = [100_u8; 4];
    assert!(!filter_simple_edge(&mut short, 1, 1, 10));
    assert!(!filter_normal_edge(&mut short, 2, 1, strength, true));
    assert!(!filter_simple_edge(&mut short, 2, 0, 10));

    let mut sharp = [0, 0, 0, 0, 255, 255, 255, 255];
    assert!(!filter_normal_edge(&mut sharp, 4, 1, strength, true));
    assert_eq!(sharp, [0, 0, 0, 0, 255, 255, 255, 255]);
}

#[test]
fn strength_controls_internal_filtering() {
    let disabled = LoopFilterStrength::default();
    assert!(!disabled.filters_inner(true, false));
    let enabled = LoopFilterStrength {
        edge_limit: 1,
        ..disabled
    };
    assert!(enabled.filters_inner(true, true));
    assert!(enabled.filters_inner(false, false));
    assert!(!enabled.filters_inner(false, true));
}
