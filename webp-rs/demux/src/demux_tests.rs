//! Tests for top-level WebP demuxing.

use super::*;
use crate::ANIM;
use crate::ANMF;

fn limits() -> ContainerLimits {
    ContainerLimits::default()
}

fn vp8(width: u16, height: u16) -> Vec<u8> {
    let mut payload = vec![0x10, 0, 0, 0x9d, 0x01, 0x2a];
    payload.extend_from_slice(&width.to_le_bytes());
    payload.extend_from_slice(&height.to_le_bytes());
    payload
}

fn vp8l(width: u32, height: u32, alpha_hint: bool) -> Vec<u8> {
    let fields = (width - 1) | ((height - 1) << 14) | (u32::from(alpha_hint) << 28);
    let mut payload = vec![0x2f];
    payload.extend_from_slice(&fields.to_le_bytes());
    payload
}

fn riff(chunks: &[(FourCc, &[u8], Option<u8>)]) -> Vec<u8> {
    let mut body = b"WEBP".to_vec();
    for (fourcc, payload, padding) in chunks {
        body.extend_from_slice(fourcc);
        body.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        body.extend_from_slice(payload);
        if payload.len() % 2 == 1 {
            body.push(padding.unwrap_or(0));
        }
    }
    let mut output = b"RIFF".to_vec();
    output.extend_from_slice(&(body.len() as u32).to_le_bytes());
    output.extend_from_slice(&body);
    output
}

#[test]
fn every_riff_prefix_is_an_error_except_the_complete_file() {
    let payload = vp8(1, 1);
    let valid = riff(&[(VP8, &payload, None)]);
    for prefix in 0..valid.len() {
        assert!(
            parse(
                &valid[..prefix],
                CompatibilityProfile::SpecStrict,
                &limits()
            )
            .is_err(),
            "prefix {prefix}"
        );
    }
    assert!(parse(&valid, CompatibilityProfile::SpecStrict, &limits()).is_ok());
}

#[test]
fn odd_padding_is_checked_by_profile() {
    let payload = vp8l(1, 1, false);
    let valid = riff(&[(VP8L, &payload, Some(0))]);
    assert_eq!(
        parse(&valid, CompatibilityProfile::SpecStrict, &limits())
            .unwrap()
            .chunks()[0]
            .padding,
        Some(0)
    );
    let non_zero = riff(&[(VP8L, &payload, Some(8))]);
    assert!(parse(&non_zero, CompatibilityProfile::SpecStrict, &limits()).is_err());
    assert!(
        parse(
            &non_zero,
            CompatibilityProfile::LibwebpCompatible,
            &limits()
        )
        .is_ok()
    );
}

#[test]
fn compatible_profile_preserves_trailing_and_unknown_chunks() {
    let payload = vp8(1, 1);
    let mut bytes = riff(&[(*b"zZZ!", &[7, 0, 8], Some(0)), (VP8, &payload, None)]);
    bytes.extend_from_slice(&[0xaa, 0xbb]);
    assert!(parse(&bytes, CompatibilityProfile::SpecStrict, &limits()).is_err());
    let parsed = parse(&bytes, CompatibilityProfile::LibwebpCompatible, &limits()).unwrap();
    assert_eq!(parsed.trailing(), &[0xaa, 0xbb]);
    let unknown: Vec<_> = parsed.unknown_chunks().collect();
    assert_eq!(unknown.len(), 1);
    assert_eq!(unknown[0].fourcc, *b"zZZ!");
    assert_eq!(unknown[0].payload, &[7, 0, 8]);
}

#[test]
fn truncated_large_chunk_size_does_not_overrun() {
    let mut bytes = b"RIFF".to_vec();
    bytes.extend_from_slice(&12u32.to_le_bytes());
    bytes.extend_from_slice(b"WEBPVP8 ");
    bytes.extend_from_slice(&u32::MAX.to_le_bytes());
    assert_eq!(
        parse(&bytes, CompatibilityProfile::SpecStrict, &limits())
            .unwrap_err()
            .kind(),
        ContainerErrorKind::UnexpectedEof
    );
}

#[test]
fn truncated_large_animation_subchunk_does_not_overrun() {
    let vp8x = [1 << 1, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    let mut anmf = vec![0; 16];
    anmf.extend_from_slice(b"VP8L");
    anmf.extend_from_slice(&u32::MAX.to_le_bytes());
    anmf.push(0);
    let bytes = riff(&[
        (VP8X, &vp8x, None),
        (ANIM, &[0; 6], None),
        (ANMF, &anmf, Some(0)),
    ]);

    assert_eq!(
        parse(&bytes, CompatibilityProfile::SpecStrict, &limits())
            .unwrap_err()
            .kind(),
        ContainerErrorKind::UnexpectedEof
    );
}

#[test]
fn vp8x_parses_canvas_and_extracts_raw_metadata() {
    let vp8x = [0b0010_1100, 0, 0, 0, 4, 0, 0, 2, 0, 0];
    let payload = vp8(5, 3);
    let bytes = riff(&[
        (VP8X, &vp8x, None),
        (ICCP, &[1, 0], None),
        (EXIF, &[0xff], Some(0)),
        (XMP, b"x", Some(0)),
        (VP8, &payload, None),
    ]);
    let parsed = parse(&bytes, CompatibilityProfile::SpecStrict, &limits()).unwrap();
    assert_eq!(parsed.vp8x().unwrap().canvas_width, 5);
    assert_eq!(parsed.vp8x().unwrap().canvas_height, 3);
    assert_eq!(
        parsed.metadata(),
        Metadata {
            iccp: Some(&[1, 0]),
            exif: Some(&[0xff]),
            xmp: Some(b"x")
        }
    );
}

#[test]
fn strict_rejects_vp8x_metadata_flag_mismatch() {
    let vp8x = [0; 10];
    let payload = vp8(1, 1);
    let bytes = riff(&[
        (VP8X, &vp8x, None),
        (EXIF, &[1], Some(0)),
        (VP8, &payload, None),
    ]);
    assert!(parse(&bytes, CompatibilityProfile::SpecStrict, &limits()).is_err());
    assert!(parse(&bytes, CompatibilityProfile::LibwebpCompatible, &limits()).is_ok());
}

#[test]
fn readers_ignore_reserved_vp8x_bits_in_both_profiles() {
    let vp8x = [0b1000_0000, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    let payload = vp8(1, 1);
    let bytes = riff(&[(VP8X, &vp8x, None), (VP8, &payload, None)]);
    assert!(parse(&bytes, CompatibilityProfile::SpecStrict, &limits()).is_ok());
    assert!(parse(&bytes, CompatibilityProfile::LibwebpCompatible, &limits()).is_ok());
}

#[test]
fn chunk_limit_is_checked_before_chunk_storage_grows() {
    let payload = vp8(1, 1);
    let bytes = riff(&[(VP8, &payload, None)]);
    let limits = ContainerLimits {
        max_chunks: 0,
        ..limits()
    };
    assert_eq!(
        parse(&bytes, CompatibilityProfile::SpecStrict, &limits)
            .unwrap_err()
            .kind(),
        ContainerErrorKind::LimitExceeded
    );
}

#[test]
fn strict_requires_image_data_and_reconstruction_order() {
    assert!(parse(&riff(&[]), CompatibilityProfile::SpecStrict, &limits()).is_err());

    let empty_vp8x = [0; 10];
    assert!(
        parse(
            &riff(&[(VP8X, &empty_vp8x, None)]),
            CompatibilityProfile::SpecStrict,
            &limits()
        )
        .is_err()
    );

    let vp8x = [1 << 5, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    let payload = vp8(1, 1);
    let iccp_after_image = riff(&[
        (VP8X, &vp8x, None),
        (VP8, &payload, None),
        (ICCP, &[1], Some(0)),
    ]);
    assert!(
        parse(
            &iccp_after_image,
            CompatibilityProfile::SpecStrict,
            &limits()
        )
        .is_err()
    );

    let animation_vp8x = [1 << 1, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    let mut frame = vec![0; 16];
    frame.extend_from_slice(b"VP8 ");
    frame.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    frame.extend_from_slice(&payload);
    let frame_before_control = riff(&[
        (VP8X, &animation_vp8x, None),
        (ANMF, &frame, None),
        (ANIM, &[0; 6], None),
    ]);
    assert!(
        parse(
            &frame_before_control,
            CompatibilityProfile::SpecStrict,
            &limits()
        )
        .is_err()
    );
}

#[test]
fn simple_images_expose_canvas_and_obey_dimension_limits() {
    let payload = vp8l(7, 9, true);
    let bytes = riff(&[(VP8L, &payload, Some(0))]);
    let parsed = parse(&bytes, CompatibilityProfile::SpecStrict, &limits()).unwrap();
    assert_eq!(parsed.canvas_dimensions(), Some((7, 9)));
    assert_eq!(parsed.frame_count(), 1);
    assert!(!parsed.is_animated());
    assert!(parsed.image().unwrap().has_alpha_hint());

    let limits = ContainerLimits {
        max_width: 6,
        ..limits()
    };
    assert_eq!(
        parse(&bytes, CompatibilityProfile::SpecStrict, &limits)
            .unwrap_err()
            .kind(),
        ContainerErrorKind::LimitExceeded
    );
}

#[test]
fn extended_canvas_must_match_static_bitstream_dimensions() {
    let vp8x = [0; 10];
    let payload = vp8(2, 1);
    let bytes = riff(&[(VP8X, &vp8x, None), (VP8, &payload, None)]);
    assert_eq!(
        parse(&bytes, CompatibilityProfile::SpecStrict, &limits())
            .unwrap_err()
            .kind(),
        ContainerErrorKind::InvalidDimensions
    );

    let vp8x = [0, 0, 0, 0, 6, 0, 0, 8, 0, 0];
    let payload = vp8l(7, 9, true);
    let bytes = riff(&[(VP8X, &vp8x, None), (VP8L, &payload, Some(0))]);
    assert_eq!(
        parse(&bytes, CompatibilityProfile::SpecStrict, &limits())
            .unwrap_err()
            .kind(),
        ContainerErrorKind::InvalidContainer
    );
}
