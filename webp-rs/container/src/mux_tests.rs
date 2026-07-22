use super::*;

#[test]
fn static_vp8l_serialization_preserves_chunk_order_and_padding() {
    let encoded = serialize_vp8l(
        vec![1, 2, 3],
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
        webp_core::CompatibilityProfile::SpecStrict,
        &webp_core::DecodeLimits::default(),
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
