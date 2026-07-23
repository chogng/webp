use super::super::*;

fn patterned_rgba(width: usize, height: usize, transparent: bool) -> Vec<u8> {
    let mut rgba = Vec::with_capacity(width * height * 4);
    for y in 0..height {
        for x in 0..width {
            rgba.extend_from_slice(&[
                x.wrapping_mul(13).wrapping_add(y * 3) as u8,
                x.wrapping_mul(5).wrapping_add(y * 17) as u8,
                x.wrapping_mul(29).wrapping_add(y * 7) as u8,
                if transparent {
                    x.wrapping_add(y * 11) as u8
                } else {
                    u8::MAX
                },
            ]);
        }
    }
    rgba
}

#[test]
fn exact_single_estimate_matches_written_bits_payload_and_riff() {
    for (width, height, transparent) in [
        (1, 1, true),
        (127, 3, false),
        (128, 3, true),
        (129, 3, false),
        (255, 2, true),
        (256, 2, false),
        (257, 2, true),
        (16_384, 1, true),
    ] {
        let rgba = patterned_rgba(width, height, transparent);
        let (predicted_bits, written_bits, predicted_payload_bytes, predicted_riff_bytes) =
            spatial_writer::single_estimate_for_test(width as u32, height as u32, &rgba)
                .expect("estimate same-profile single");
        let actual = spatial_writer::encode_single_for_test(width as u32, height as u32, &rgba)
            .expect("write same-profile single");
        let actual_payload_bytes = u32::from_le_bytes(actual[16..20].try_into().unwrap()) as usize;
        assert_eq!(predicted_bits, written_bits, "{width} by {height}");
        assert_eq!(predicted_payload_bytes, actual_payload_bytes);
        assert_eq!(predicted_riff_bytes, actual.len());
    }
}

#[test]
fn exact_single_estimate_counts_copy_extra_bits() {
    for length in [3, 4, 5, 17, 18, 33, 65, 129, 300, 4096] {
        let rgba = [7, 11, 19, 127].repeat(length);
        let (predicted_bits, written_bits, _, predicted_riff_bytes) =
            spatial_writer::single_estimate_for_test(length as u32, 1, &rgba)
                .expect("estimate copy stream");
        let actual = spatial_writer::encode_single_for_test(length as u32, 1, &rgba)
            .expect("write copy stream");
        assert_eq!(predicted_bits, written_bits, "copy length {length}");
        assert_eq!(predicted_riff_bytes, actual.len(), "copy length {length}");
    }
}

#[test]
fn exact_single_estimate_covers_both_riff_padding_parities() {
    let mut saw_even = false;
    let mut saw_odd = false;
    for width in 1..=128 {
        let rgba = patterned_rgba(width, 1, true);
        let (_, _, payload_bytes, riff_bytes) =
            spatial_writer::single_estimate_for_test(width as u32, 1, &rgba)
                .expect("estimate padding case");
        let actual = spatial_writer::encode_single_for_test(width as u32, 1, &rgba)
            .expect("write padding case");
        assert_eq!(riff_bytes, actual.len());
        saw_even |= payload_bytes.is_multiple_of(2);
        saw_odd |= !payload_bytes.is_multiple_of(2);
    }
    assert!(saw_even && saw_odd);
}

#[test]
fn exact_spatial_estimate_matches_group_map_tables_tokens_and_riff() {
    for profile in [
        spatial_plan::SpatialProfile::Compact,
        spatial_plan::SpatialProfile::LowLatency,
    ] {
        for (width, height, transparent) in [
            (1, 1, true),
            (127, 3, false),
            (128, 3, true),
            (129, 3, false),
            (511, 5, true),
        ] {
            let rgba = patterned_rgba(width, height, transparent);
            let (predicted_bits, written_bits, predicted_payload_bytes, predicted_riff_bytes) =
                spatial_writer::candidate_estimate_for_test(
                    width as u32,
                    height as u32,
                    &rgba,
                    profile,
                )
                .expect("estimate spatial candidate");
            let actual = spatial_writer::encode_candidate_for_test(
                width as u32,
                height as u32,
                &rgba,
                profile,
            )
            .expect("write spatial candidate");
            let actual_payload_bytes =
                u32::from_le_bytes(actual[16..20].try_into().unwrap()) as usize;
            assert_eq!(predicted_bits, written_bits);
            assert_eq!(predicted_payload_bytes, actual_payload_bytes);
            assert_eq!(predicted_riff_bytes, actual.len());
        }
    }
}

#[test]
fn exact_selection_skips_every_losing_payload_and_preserves_fallbacks() {
    let tiny = [17, 29, 43, 91];
    let single = spatial_writer::encode_single_for_test(1, 1, &tiny).expect("encode tiny single");
    let (selected, stats) = spatial_writer::encode_profile_exact_for_test(
        1,
        1,
        &tiny,
        spatial_plan::SpatialProfile::Compact,
    )
    .expect("select tiny stream");
    assert_eq!(selected, single);
    assert_eq!(stats.predicted_riff_bytes, Some(single.len()));
    assert!(stats.predicted_payload_bits.is_some());
    assert!(stats.predicted_payload_bytes.is_some());
    assert!(stats.predicted_candidate_payload_bits.is_some());
    assert!(stats.predicted_candidate_riff_bytes.is_some());
    assert!(!stats.losing_single_main_written);
    assert!(!stats.losing_candidate_main_written);
    assert!(!stats.estimator_fallback);
    assert!(!stats.candidate_won);
    assert!(!spatial_writer::candidate_wins(single.len(), single.len()));

    let control = spatial_writer::encode_profile_control_for_test(
        1,
        1,
        &tiny,
        spatial_plan::SpatialProfile::Compact,
    )
    .expect("encode current-main control");
    let (fallback, stats) = spatial_writer::encode_profile_plan_fallback_for_test(
        1,
        1,
        &tiny,
        spatial_plan::SpatialProfile::Compact,
    )
    .expect("fall back after a plan failure");
    assert_eq!(fallback, control);
    assert!(stats.predicted_payload_bits.is_none());
    assert!(stats.predicted_payload_bytes.is_none());
    assert!(stats.predicted_riff_bytes.is_none());
    assert!(stats.predicted_candidate_payload_bits.is_none());
    assert!(stats.predicted_candidate_riff_bytes.is_none());
    assert!(stats.losing_single_main_written);
    assert!(stats.losing_candidate_main_written);
    assert!(stats.estimator_fallback);
    assert!(!stats.candidate_won);

    let width = 1024_usize;
    let height = 1024_usize;
    let mut rgba = Vec::with_capacity(width * height * 4);
    for y in 0..height {
        for x in 0..width {
            let region = (x / 256 + (y / 256) * 4) as u8;
            let noise = ((x.wrapping_mul(37) ^ y.wrapping_mul(101) ^ (x * y)) & 7) as u8;
            let green = region.wrapping_mul(16).wrapping_add(noise);
            rgba.extend_from_slice(&[
                green.wrapping_add(region.wrapping_mul(3)),
                green,
                green.wrapping_add(region.wrapping_mul(5)),
                255,
            ]);
        }
    }
    let (selected, stats) = spatial_writer::encode_profile_exact_for_test(
        width as u32,
        height as u32,
        &rgba,
        spatial_plan::SpatialProfile::Compact,
    )
    .expect("select coarse stream");
    let candidate = spatial_writer::encode_candidate_for_test(
        width as u32,
        height as u32,
        &rgba,
        spatial_plan::SpatialProfile::Compact,
    )
    .expect("encode coarse stream");
    assert_eq!(selected, candidate);
    assert!(!stats.losing_single_main_written);
    assert!(!stats.losing_candidate_main_written);
    assert!(!stats.estimator_fallback);
    assert!(stats.candidate_won);
}
