use super::*;

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
fn frame_headers_and_nested_chunks_are_validated() {
    let vp8x = [0b0000_0010, 0, 0, 0, 3, 0, 0, 2, 0, 0];
    let mut frame = vec![1, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 7, 0, 0, 0b11];
    frame.extend_from_slice(b"VP8 ");
    frame.extend_from_slice(&2_u32.to_le_bytes());
    frame.extend_from_slice(&[9, 8]);
    let bytes = riff(&[
        (VP8X, &vp8x, None),
        (ANIM, &[1, 2, 3, 4, 2, 0], None),
        (ANMF, &frame, None),
    ]);
    let parsed = parse(
        &bytes,
        CompatibilityProfile::SpecStrict,
        &ContainerLimits::default(),
    )
    .unwrap();
    let animation = parsed.animation().unwrap();
    assert_eq!(animation.background_bgra, [1, 2, 3, 4]);
    assert_eq!(animation.loop_count, 2);
    assert_eq!(
        animation.frames(),
        &[AnimationFrame {
            x: 2,
            y: 0,
            width: 2,
            height: 1,
            duration_ms: 7,
            dispose_to_background: true,
            blend: false,
            alpha: None,
            bitstream: FrameBitstream::Vp8(&[9, 8])
        }]
    );
}

#[test]
fn frame_layout_and_resource_limits_are_rejected() {
    let vp8x = [0b0000_0010, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    let mut out_of_bounds = vec![1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    out_of_bounds.extend_from_slice(b"VP8 ");
    out_of_bounds.extend_from_slice(&0_u32.to_le_bytes());
    let invalid = riff(&[
        (VP8X, &vp8x, None),
        (ANIM, &[0; 6], None),
        (ANMF, &out_of_bounds, None),
    ]);
    assert_eq!(
        parse(
            &invalid,
            CompatibilityProfile::SpecStrict,
            &ContainerLimits::default()
        )
        .unwrap_err()
        .kind(),
        ContainerErrorKind::InvalidContainer
    );
    let mut frame = vec![0; 16];
    frame.extend_from_slice(b"VP8 ");
    frame.extend_from_slice(&0_u32.to_le_bytes());
    let valid = riff(&[
        (VP8X, &vp8x, None),
        (ANIM, &[0; 6], None),
        (ANMF, &frame, None),
    ]);
    let limits = ContainerLimits {
        max_frames: 0,
        ..ContainerLimits::default()
    };
    assert_eq!(
        parse(&valid, CompatibilityProfile::SpecStrict, &limits)
            .unwrap_err()
            .kind(),
        ContainerErrorKind::LimitExceeded
    );
}

#[test]
fn anmf_alpha_requires_the_vp8x_alpha_flag_in_strict_mode() {
    let vp8x_without_alpha = [0b0000_0010, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    let mut frame = vec![0; 16];
    frame.extend_from_slice(b"ALPH");
    frame.extend_from_slice(&1_u32.to_le_bytes());
    frame.push(0);
    frame.push(0);
    frame.extend_from_slice(b"VP8 ");
    frame.extend_from_slice(&0_u32.to_le_bytes());
    let bytes = riff(&[
        (VP8X, &vp8x_without_alpha, None),
        (ANIM, &[0; 6], None),
        (ANMF, &frame, None),
    ]);
    assert_eq!(
        parse(
            &bytes,
            CompatibilityProfile::SpecStrict,
            &ContainerLimits::default()
        )
        .unwrap_err()
        .kind(),
        ContainerErrorKind::InvalidContainer
    );
    assert!(
        parse(
            &bytes,
            CompatibilityProfile::LibwebpCompatible,
            &ContainerLimits::default()
        )
        .is_ok()
    );
}
