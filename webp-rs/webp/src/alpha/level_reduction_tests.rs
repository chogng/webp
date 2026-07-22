use super::*;

#[test]
fn quality_controls_level_count_and_preserves_extrema() {
    let samples = (0_u8..=255).collect::<Vec<_>>();
    let reduced = quantize(&samples, 0).unwrap();
    assert_eq!(reduced.first(), Some(&0));
    assert_eq!(reduced.last(), Some(&255));
    assert!(reduced.windows(2).all(|pair| pair[0] <= pair[1]));
    assert!(
        reduced
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            <= 2
    );

    let moderate = quantize(&samples, 70).unwrap();
    assert!(
        moderate
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            <= 16
    );
}
