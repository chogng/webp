//! Tests for alpha payload forward filters.

use super::*;

#[test]
fn fast_estimate_finds_horizontal_ramp() {
    let samples = (0..64)
        .flat_map(|_| (0..64).map(|x| (x * 3) as u8))
        .collect::<Vec<_>>();
    assert_eq!(
        candidates(&samples, 64, 64, AlphaFilterSelection::Fast),
        [AlphaFilter::None, AlphaFilter::Horizontal]
    );
}

#[test]
fn forward_filters_match_known_residuals() {
    let samples = [1, 2, 3, 4, 5, 6];
    assert_eq!(
        apply(&samples, 3, AlphaFilter::Horizontal).unwrap(),
        [1, 1, 1, 3, 1, 1]
    );
    assert_eq!(
        apply(&samples, 3, AlphaFilter::Vertical).unwrap(),
        [1, 1, 1, 3, 3, 3]
    );
    assert_eq!(
        apply(&samples, 3, AlphaFilter::Gradient).unwrap(),
        [1, 1, 1, 3, 0, 0]
    );
}
