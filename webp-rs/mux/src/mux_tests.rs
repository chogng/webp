//! Tests for WebP muxing.

use crate::*;

fn vp8l_payload(width: u32, height: u32, has_alpha: bool) -> Vec<u8> {
    let fields = (width - 1) | ((height - 1) << 14) | (u32::from(has_alpha) << 28);
    let mut payload = vec![0x2f];
    payload.extend_from_slice(&fields.to_le_bytes());
    payload
}

#[test]
fn static_vp8l_serialization_preserves_chunk_order_and_padding() {
    let encoded = serialize_vp8l(
        vp8l_payload(2, 3, true),
        2,
        3,
        true,
        Metadata {
            iccp: Some(b"i"),
            exif: Some(b"ex"),
            xmp: Some(b"x"),
        },
    )
    .unwrap();
    assert_eq!(&encoded[..4], b"RIFF");
    assert_eq!(&encoded[8..12], b"WEBP");
    let parsed = crate::parse(
        &encoded,
        crate::CompatibilityProfile::SpecStrict,
        &crate::ContainerLimits::default(),
    )
    .unwrap();
    let fourccs = parsed
        .chunks()
        .iter()
        .map(|chunk| chunk.fourcc)
        .collect::<Vec<_>>();
    assert_eq!(
        fourccs,
        [
            crate::VP8X,
            crate::ICCP,
            crate::VP8L,
            crate::EXIF,
            crate::XMP
        ]
    );
}

#[test]
fn animation_frame_serialization_keeps_wire_geometry_and_flags() {
    let frame = serialize_animation_frame(AnimationFrameMux {
        x: 4,
        y: 6,
        width: 3,
        height: 5,
        duration_ms: 17,
        dispose_to_background: true,
        blend: false,
        vp8l_payload: &[1, 2, 3],
    })
    .unwrap();
    assert_eq!(&frame[..3], &[2, 0, 0]);
    assert_eq!(&frame[3..6], &[3, 0, 0]);
    assert_eq!(&frame[6..9], &[2, 0, 0]);
    assert_eq!(&frame[9..12], &[4, 0, 0]);
    assert_eq!(&frame[12..15], &[17, 0, 0]);
    assert_eq!(frame[15], 0b11);
    assert_eq!(&frame[16..20], b"VP8L");
}

#[test]
fn serializers_reject_dimensions_above_the_wire_limit() {
    const ABOVE_LIMIT: u32 = (1 << 24) + 1;

    for (width, height) in [(ABOVE_LIMIT, 1), (1, ABOVE_LIMIT)] {
        assert_eq!(
            serialize_vp8l(
                Vec::new(),
                width,
                height,
                false,
                Metadata {
                    iccp: Some(b"profile"),
                    ..Metadata::default()
                },
            )
            .unwrap_err()
            .kind(),
            ContainerErrorKind::InvalidDimensions
        );
        assert_eq!(
            serialize_vp8(Vec::new(), width, height, Some(&[]))
                .unwrap_err()
                .kind(),
            ContainerErrorKind::InvalidDimensions
        );
        assert_eq!(
            serialize_animation(
                width,
                height,
                AnimationMuxOptions {
                    background_rgba: [0; 4],
                    loop_count: 0,
                },
                false,
                &[],
                Metadata::default(),
            )
            .unwrap_err()
            .kind(),
            ContainerErrorKind::InvalidDimensions
        );
        assert_eq!(
            serialize_animation_frame(AnimationFrameMux {
                x: 0,
                y: 0,
                width,
                height,
                duration_ms: 0,
                dispose_to_background: false,
                blend: true,
                vp8l_payload: &[],
            })
            .unwrap_err()
            .kind(),
            ContainerErrorKind::InvalidAnimation
        );
    }
}

#[test]
fn maximum_wire_dimensions_are_encoded_without_truncation() {
    const MAX_DIMENSION: u32 = 1 << 24;

    let vp8l = serialize_vp8l(
        Vec::new(),
        MAX_DIMENSION,
        MAX_DIMENSION,
        false,
        Metadata {
            iccp: Some(b"profile"),
            ..Metadata::default()
        },
    )
    .unwrap();
    assert_eq!(&vp8l[24..30], &[0xff; 6]);

    let vp8 = serialize_vp8(Vec::new(), MAX_DIMENSION, MAX_DIMENSION, Some(&[])).unwrap();
    assert_eq!(&vp8[24..30], &[0xff; 6]);

    let animation = serialize_animation(
        MAX_DIMENSION,
        MAX_DIMENSION,
        AnimationMuxOptions {
            background_rgba: [0; 4],
            loop_count: 0,
        },
        false,
        &[],
        Metadata::default(),
    )
    .unwrap();
    assert_eq!(&animation[24..30], &[0xff; 6]);

    let frame = serialize_animation_frame(AnimationFrameMux {
        x: 0,
        y: 0,
        width: MAX_DIMENSION,
        height: MAX_DIMENSION,
        duration_ms: 0,
        dispose_to_background: false,
        blend: true,
        vp8l_payload: &[],
    })
    .unwrap();
    assert_eq!(&frame[6..12], &[0xff; 6]);
}
