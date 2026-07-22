use webp::AnimationEncodeFrame;
use webp::AnimationEncodeOptions;
use webp::DecodeLimits;
use webp::DecodeOptions;
use webp::Metadata;
use webp::decode_animation;
use webp::encode_lossless_animation;
use webp::encode_lossless_animation_with_metadata;
use webp::read_metadata;

#[test]
fn lossless_animation_encoder_preserves_rectangles_composition_and_wire_flags() {
    let first = [10, 20, 30, 255, 40, 50, 60, 255];
    let second = [100, 110, 120, 128];
    let options = AnimationEncodeOptions {
        background_rgba: [4, 3, 2, 1],
        loop_count: 7,
    };
    let encoded = encode_lossless_animation(
        3,
        1,
        &[
            AnimationEncodeFrame {
                x: 0,
                y: 0,
                width: 2,
                height: 1,
                duration_ms: 10,
                rgba: &first,
                dispose_to_background: true,
                blend: false,
            },
            AnimationEncodeFrame {
                x: 2,
                y: 0,
                width: 1,
                height: 1,
                duration_ms: 20,
                rgba: &second,
                dispose_to_background: false,
                blend: false,
            },
            AnimationEncodeFrame {
                x: 2,
                y: 0,
                width: 1,
                height: 1,
                duration_ms: 30,
                rgba: &[7, 8, 9, 255],
                dispose_to_background: false,
                blend: true,
            },
        ],
        options,
    )
    .expect("encode lossless animation");

    let parsed = webp_container::parse(
        &encoded,
        webp_container::CompatibilityProfile::SpecStrict,
        &webp_container::ContainerLimits::default(),
    )
    .expect("strictly parse encoded animation");
    let vp8x = parsed.vp8x().expect("animation has VP8X");
    assert!(vp8x.flags.animation());
    assert!(vp8x.flags.alpha());
    let animation = parsed.animation().expect("animation controls");
    assert_eq!(animation.background_bgra, [2, 3, 4, 1]);
    assert_eq!(animation.loop_count, 7);
    assert_eq!(animation.frames().len(), 3);
    assert_eq!(animation.frames()[0].duration_ms, 10);
    assert!(animation.frames()[0].dispose_to_background);
    assert!(!animation.frames()[0].blend);
    assert_eq!((animation.frames()[1].x, animation.frames()[1].y), (2, 0));
    assert_eq!(animation.frames()[1].duration_ms, 20);
    assert!(animation.frames()[2].blend);
    assert_eq!(animation.frames()[2].duration_ms, 30);

    let decoded = decode_animation(&encoded, &DecodeOptions::default())
        .expect("decode encoded lossless animation");
    assert_eq!(
        (decoded.width, decoded.height, decoded.loop_count),
        (3, 1, 7)
    );
    assert_eq!(decoded.frames.len(), 3);
    assert_eq!(decoded.frames[0].duration_ms, 10);
    assert_eq!(
        decoded.frames[0].rgba,
        [10, 20, 30, 255, 40, 50, 60, 255, 4, 3, 2, 1]
    );
    assert_eq!(decoded.frames[1].duration_ms, 20);
    assert_eq!(
        decoded.frames[1].rgba,
        [4, 3, 2, 1, 4, 3, 2, 1, 100, 110, 120, 128]
    );
    assert_eq!(decoded.frames[2].duration_ms, 30);
    assert_eq!(
        decoded.frames[2].rgba,
        [4, 3, 2, 1, 4, 3, 2, 1, 7, 8, 9, 255]
    );
}

#[test]
fn lossless_animation_encoder_rejects_invalid_frame_geometry_and_timing() {
    let rgba = [0_u8; 4];
    let valid_frame = AnimationEncodeFrame {
        x: 0,
        y: 0,
        width: 1,
        height: 1,
        duration_ms: 0,
        rgba: &rgba,
        dispose_to_background: false,
        blend: true,
    };
    assert_eq!(
        encode_lossless_animation(1, 1, &[], AnimationEncodeOptions::default()).unwrap_err(),
        webp::EncodeError::InvalidAnimation
    );
    assert_eq!(
        encode_lossless_animation(
            2,
            1,
            &[AnimationEncodeFrame {
                x: 1,
                ..valid_frame
            }],
            AnimationEncodeOptions::default(),
        )
        .unwrap_err(),
        webp::EncodeError::InvalidAnimation
    );
    assert_eq!(
        encode_lossless_animation(
            1,
            1,
            &[AnimationEncodeFrame {
                duration_ms: 1 << 24,
                ..valid_frame
            }],
            AnimationEncodeOptions::default(),
        )
        .unwrap_err(),
        webp::EncodeError::InvalidAnimation
    );
}

#[test]
fn lossless_animation_encoder_muxes_raw_metadata() {
    let rgba = [1, 2, 3, 255];
    let metadata = Metadata {
        iccp: Some(vec![0, 1, 2]),
        exif: Some(vec![3, 4]),
        xmp: Some(b"<xmp/>".to_vec()),
    };
    let encoded = encode_lossless_animation_with_metadata(
        1,
        1,
        &[AnimationEncodeFrame {
            x: 0,
            y: 0,
            width: 1,
            height: 1,
            duration_ms: 1,
            rgba: &rgba,
            dispose_to_background: false,
            blend: false,
        }],
        AnimationEncodeOptions::default(),
        &metadata,
    )
    .expect("encode metadata animation");
    assert_eq!(
        read_metadata(&encoded, &DecodeLimits::default()).expect("read animation metadata"),
        metadata
    );
    assert_eq!(
        decode_animation(&encoded, &DecodeOptions::default())
            .expect("decode metadata animation")
            .frames[0]
            .rgba,
        rgba
    );
}
