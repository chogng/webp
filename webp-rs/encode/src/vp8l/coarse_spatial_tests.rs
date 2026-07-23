use super::*;
use crate::static_image::copy_vp8l_payload;
use crate::static_image::wrap_vp8l_with_metadata;
use crate::{
    LosslessEncodeOptions, LosslessEncodeProfile, encode_lossless_rgba,
    encode_lossless_rgba_with_metadata, encode_lossless_rgba_with_metadata_and_options,
    encode_lossless_rgba_with_options,
};
use webp_decode::{DecodeLimits, DecodeOptions, Metadata, decode, read_metadata};

fn options(profile: LosslessEncodeProfile) -> LosslessEncodeOptions {
    LosslessEncodeOptions { profile }
}

fn profiles() -> [LosslessEncodeProfile; 2] {
    [
        LosslessEncodeProfile::FastDecodeCompact,
        LosslessEncodeProfile::FastDecodeLowLatency,
    ]
}

fn spatial_profile(profile: LosslessEncodeProfile) -> spatial_plan::SpatialProfile {
    match profile {
        LosslessEncodeProfile::FastDecodeCompact => spatial_plan::SpatialProfile::Compact,
        LosslessEncodeProfile::FastDecodeLowLatency => spatial_plan::SpatialProfile::LowLatency,
        LosslessEncodeProfile::Default | LosslessEncodeProfile::HighCompression => {
            panic!("profile has no coarse spatial layout")
        }
    }
}

fn patterned_rgba(width: u32, height: u32, alpha: bool) -> Vec<u8> {
    let mut rgba = Vec::with_capacity((width as usize) * (height as usize) * 4);
    for y in 0..height {
        for x in 0..width {
            rgba.extend_from_slice(&[
                x.wrapping_mul(13).wrapping_add(y.wrapping_mul(3)) as u8,
                x.wrapping_mul(5).wrapping_add(y.wrapping_mul(17)) as u8,
                x.wrapping_mul(29).wrapping_add(y.wrapping_mul(7)) as u8,
                if alpha {
                    x.wrapping_add(y.wrapping_mul(11)) as u8
                } else {
                    u8::MAX
                },
            ]);
        }
    }
    rgba
}

#[test]
fn default_options_preserve_static_and_metadata_bytes() {
    let rgba = patterned_rgba(37, 19, true);
    let direct = encode_lossless_rgba(37, 19, &rgba).expect("encode established default");
    let with_options =
        encode_lossless_rgba_with_options(37, 19, &rgba, LosslessEncodeOptions::default())
            .expect("encode options default");
    assert_eq!(with_options, direct);

    let metadata = Metadata {
        iccp: Some(vec![1, 2, 3]),
        exif: Some(vec![4, 5]),
        xmp: Some(b"<xmp/>".to_vec()),
    };
    let direct = encode_lossless_rgba_with_metadata(37, 19, &rgba, &metadata)
        .expect("encode established metadata default");
    let with_options = encode_lossless_rgba_with_metadata_and_options(
        37,
        19,
        &rgba,
        &metadata,
        LosslessEncodeOptions::default(),
    )
    .expect("encode metadata options default");
    assert_eq!(with_options, direct);
}

#[test]
fn product_profiles_round_trip_tiny_transparent_and_block_boundaries() {
    for (width, height) in [
        (1, 1),
        (127, 129),
        (128, 128),
        (129, 127),
        (255, 257),
        (256, 256),
        (257, 255),
    ] {
        let rgba = patterned_rgba(width, height, true);
        for profile in profiles() {
            let encoded = encode_lossless_rgba_with_options(width, height, &rgba, options(profile))
                .expect("encode product profile");
            let image = decode(&encoded, &DecodeOptions::default())
                .expect("decode product profile with project decoder");
            assert_eq!((image.width, image.height), (width, height));
            assert_eq!(image.rgba, rgba, "{profile:?} at {width} by {height}");
        }
    }
}

#[test]
fn product_profiles_fallback_to_the_exact_same_profile_single_file() {
    let rgba = [17, 29, 43, 91];
    let single =
        spatial_writer::encode_single_for_test(1, 1, &rgba).expect("encode same-profile single");
    for profile in profiles() {
        let candidate =
            spatial_writer::encode_candidate_for_test(1, 1, &rgba, spatial_profile(profile))
                .expect("encode coarse candidate");
        assert!(candidate.len() >= single.len());
        let selected = encode_lossless_rgba_with_options(1, 1, &rgba, options(profile))
            .expect("encode selected product stream");
        assert_eq!(selected, single, "{profile:?}");
    }
}

#[test]
fn product_profiles_select_the_coarse_file_when_it_is_strictly_smaller() {
    let width = 1024;
    let height = 1024;
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
    for profile in profiles() {
        let requested_candidate = spatial_writer::encode_candidate_for_test(
            width as u32,
            height as u32,
            &rgba,
            spatial_profile(profile),
        )
        .expect("encode requested coarse candidate");
        let single = spatial_writer::encode_single_for_test(width as u32, height as u32, &rgba)
            .expect("encode same-profile single");
        assert!(
            requested_candidate.len() < single.len(),
            "{profile:?}: candidate {} single {}",
            requested_candidate.len(),
            single.len()
        );
        let selected =
            encode_lossless_rgba_with_options(width as u32, height as u32, &rgba, options(profile))
                .expect("encode selected product stream");
        let (audited, stats) = spatial_writer::encode_profile_exact_for_test(
            width as u32,
            height as u32,
            &rgba,
            spatial_profile(profile),
        )
        .expect("audit portfolio selection");
        assert_eq!(selected, audited, "{profile:?}");
        let selected_profile = stats
            .selected_profile
            .expect("coarse input selects a spatial candidate");
        let selected_candidate = spatial_writer::encode_candidate_for_test(
            width as u32,
            height as u32,
            &rgba,
            selected_profile,
        )
        .expect("encode selected coarse candidate");
        assert_eq!(selected, selected_candidate, "{profile:?}");
        let costs = stats.portfolio_costs.expect("portfolio costs");
        let candidate_indices: &[usize] = match spatial_profile(profile) {
            spatial_plan::SpatialProfile::Compact => &[0, 1],
            spatial_plan::SpatialProfile::LowLatency => &[0, 2],
        };
        let policy_costs = candidate_indices
            .iter()
            .map(|&index| costs[index])
            .collect::<Vec<_>>();
        let policy_index =
            portfolio_policy::choose(spatial_profile(profile), &policy_costs).unwrap();
        assert_eq!(stats.selected_index, Some(candidate_indices[policy_index]));
        let floor = candidate_indices
            .iter()
            .map(|&index| costs[index].exact_bits)
            .min()
            .unwrap();
        let selected_cost = costs[stats.selected_index.unwrap()];
        let ceiling = match spatial_profile(profile) {
            spatial_plan::SpatialProfile::Compact => floor,
            spatial_plan::SpatialProfile::LowLatency => floor + floor / 100,
        };
        assert!(selected_cost.exact_bits <= ceiling);
    }
}

#[test]
fn copy_token_may_cross_a_coarse_block_end() {
    let rgba = [7, 11, 19, 255].repeat(300);
    let stream =
        TokenStream::collect(&rgba, 300, true, false, 0).expect("tokenize long repeated run");
    assert!(matches!(
        stream.tokens(),
        [
            EntropyToken::Literal(_),
            EntropyToken::Copy {
                length: 299,
                distance_code: 121,
            }
        ]
    ));
    for profile in profiles() {
        let candidate =
            spatial_writer::encode_candidate_for_test(300, 1, &rgba, spatial_profile(profile))
                .expect("encode boundary-crossing copy candidate");
        let image = decode(&candidate, &DecodeOptions::default())
            .expect("decode boundary-crossing copy candidate");
        assert_eq!(image.rgba, rgba, "{profile:?}");
    }
}

#[test]
fn product_profiles_preserve_metadata_and_pixels() {
    let rgba = patterned_rgba(513, 129, true);
    let metadata = Metadata {
        iccp: Some(vec![0, 1, 2, 3, 4]),
        exif: Some(vec![5, 6, 7]),
        xmp: Some(b"product-profile".to_vec()),
    };
    for profile in profiles() {
        let encoded = encode_lossless_rgba_with_metadata_and_options(
            513,
            129,
            &rgba,
            &metadata,
            options(profile),
        )
        .expect("encode product profile with metadata");
        assert_eq!(
            read_metadata(&encoded, &DecodeLimits::default()).expect("read metadata"),
            metadata
        );
        assert_eq!(
            decode(&encoded, &DecodeOptions::default())
                .expect("decode metadata product stream")
                .rgba,
            rgba,
            "{profile:?}"
        );
    }
}

#[test]
fn exact_selection_preserves_selected_payload_metadata_bytes() {
    let width = 513;
    let height = 129;
    let rgba = patterned_rgba(width, height, true);
    let metadata = Metadata {
        iccp: Some(vec![0, 1, 2, 3, 4]),
        exif: Some(vec![5, 6, 7]),
        xmp: Some(b"exact-cost-metadata".to_vec()),
    };
    for profile in profiles() {
        let exact = encode_lossless_rgba_with_metadata_and_options(
            width,
            height,
            &rgba,
            &metadata,
            options(profile),
        )
        .expect("encode exact-cost metadata stream");
        let selected = encode_lossless_rgba_with_options(width, height, &rgba, options(profile))
            .expect("encode selected profile payload");
        let selected_payload = copy_vp8l_payload(&selected).expect("copy selected payload");
        let expected = wrap_vp8l_with_metadata(selected_payload, width, height, true, &metadata)
            .expect("wrap selected metadata stream");
        assert_eq!(exact, expected, "{profile:?}");
    }
}
