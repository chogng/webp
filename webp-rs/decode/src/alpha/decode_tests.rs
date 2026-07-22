//! Tests for alpha payload reading.

use super::*;

fn decode_samples(filter: AlphaFilter, samples: &[u8]) -> Vec<u8> {
    let filter_bits = match filter {
        AlphaFilter::None => 0,
        AlphaFilter::Horizontal => 1,
        AlphaFilter::Vertical => 2,
        AlphaFilter::Gradient => 3,
    };
    let mut payload = vec![filter_bits << 2];
    payload.extend_from_slice(samples);
    decode_raw(
        &payload,
        3,
        2,
        CompatibilityProfile::SpecStrict,
        &DecodeLimits::default(),
    )
    .unwrap()
}

#[test]
fn raw_filters_recover_expected_samples() {
    assert_eq!(
        decode_samples(AlphaFilter::None, &[1, 2, 3, 4, 5, 6]),
        [1, 2, 3, 4, 5, 6]
    );
    assert_eq!(
        decode_samples(AlphaFilter::Horizontal, &[1, 1, 1, 3, 1, 1]),
        [1, 2, 3, 4, 5, 6]
    );
    assert_eq!(
        decode_samples(AlphaFilter::Vertical, &[1, 1, 1, 3, 3, 3]),
        [1, 2, 3, 4, 5, 6]
    );
    assert_eq!(
        decode_samples(AlphaFilter::Gradient, &[1, 1, 1, 3, 0, 0]),
        [1, 2, 3, 4, 5, 6]
    );
}

#[test]
fn validates_header_lengths_and_limits() {
    assert_eq!(
        parse_header(&[0b1100_0000], CompatibilityProfile::SpecStrict)
            .unwrap_err()
            .kind(),
        DecodeErrorKind::InvalidBitstream
    );
    assert!(parse_header(&[0b1100_0000], CompatibilityProfile::LibwebpCompatible).is_ok());
    assert_eq!(
        decode_raw(
            &[0],
            2,
            1,
            CompatibilityProfile::SpecStrict,
            &DecodeLimits::default()
        )
        .unwrap_err()
        .kind(),
        DecodeErrorKind::UnexpectedEof
    );
    assert_eq!(
        decode_raw(
            &[0, 1, 2, 3],
            2,
            1,
            CompatibilityProfile::SpecStrict,
            &DecodeLimits::default()
        )
        .unwrap_err()
        .kind(),
        DecodeErrorKind::InvalidBitstream
    );
    assert_eq!(
        decode_raw(
            &[1, 0],
            1,
            1,
            CompatibilityProfile::SpecStrict,
            &DecodeLimits::default()
        )
        .unwrap_err()
        .kind(),
        DecodeErrorKind::InvalidParameter
    );
    assert_eq!(
        decode(
            &[0],
            0,
            1,
            CompatibilityProfile::SpecStrict,
            &DecodeLimits::default()
        )
        .unwrap_err()
        .kind(),
        DecodeErrorKind::InvalidParameter
    );

    let input_limited = DecodeLimits {
        max_input_bytes: 1,
        ..DecodeLimits::default()
    };
    assert_eq!(
        decode(
            &[0, 1],
            1,
            1,
            CompatibilityProfile::SpecStrict,
            &input_limited
        )
        .unwrap_err()
        .kind(),
        DecodeErrorKind::LimitExceeded
    );
}

#[test]
fn work_budget_applies_before_every_alpha_sample() {
    let limits = DecodeLimits {
        max_work_units: 1,
        ..DecodeLimits::default()
    };
    assert_eq!(
        decode_raw(&[0, 1, 2], 2, 1, CompatibilityProfile::SpecStrict, &limits)
            .unwrap_err()
            .kind(),
        DecodeErrorKind::LimitExceeded
    );
}

#[test]
fn lossless_alpha_uses_the_vp8l_green_channel() {
    let mut writer = BitWriter::new();
    writer.write_bits(0x2f, 8).unwrap();
    writer.write_bits(0, 14).unwrap();
    writer.write_bits(0, 14).unwrap();
    writer.write_bits(0, 1).unwrap();
    writer.write_bits(0, 3).unwrap();
    writer.write_bits(0, 3).unwrap();
    for channel in [37_u8, 12, 56, 78, 0] {
        writer.write_bits(1, 1).unwrap();
        writer.write_bits(0, 1).unwrap();
        writer.write_bits(1, 1).unwrap();
        writer.write_bits(u32::from(channel), 8).unwrap();
    }
    let encoded = writer.into_bytes();
    let mut payload = vec![1];
    payload.extend_from_slice(&encoded[5..]);
    assert_eq!(
        decode(
            &payload,
            1,
            1,
            CompatibilityProfile::SpecStrict,
            &DecodeLimits::default()
        )
        .unwrap(),
        [37]
    );
}

#[test]
fn lossless_alpha_applies_its_declared_spatial_filter() {
    let mut writer = BitWriter::new();
    writer.write_bits(0x2f, 8).unwrap();
    writer.write_bits(2, 14).unwrap(); // 3px width
    writer.write_bits(1, 14).unwrap(); // 2px height
    writer.write_bits(0, 1).unwrap();
    writer.write_bits(0, 3).unwrap();
    writer.write_bits(0, 3).unwrap();
    for channel in [1_u8, 0, 0, 255, 0] {
        writer.write_bits(1, 1).unwrap();
        writer.write_bits(0, 1).unwrap();
        writer.write_bits(1, 1).unwrap();
        writer.write_bits(u32::from(channel), 8).unwrap();
    }
    let encoded = writer.into_bytes();
    let mut payload = vec![0b0101]; // lossless compression + horizontal filter
    payload.extend_from_slice(&encoded[5..]);
    assert_eq!(
        decode(
            &payload,
            3,
            2,
            CompatibilityProfile::SpecStrict,
            &DecodeLimits::default()
        )
        .unwrap(),
        [1, 2, 3, 2, 3, 4]
    );
}
