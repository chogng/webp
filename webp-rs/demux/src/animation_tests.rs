//! Tests for borrowed animation demuxing.

use super::*;
use crate::FourCc;
use crate::VP8X;
use crate::parse;

fn vp8(width: u16, height: u16) -> Vec<u8> {
    let mut payload = vec![0x10, 0, 0, 0x9d, 0x01, 0x2a];
    payload.extend_from_slice(&width.to_le_bytes());
    payload.extend_from_slice(&height.to_le_bytes());
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
fn frame_headers_and_nested_chunks_are_validated() {
    let vp8x = [0b0000_0010, 0, 0, 0, 3, 0, 0, 2, 0, 0];
    let mut frame = vec![1, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 7, 0, 0, 0b1111_1111];
    let payload = vp8(2, 1);
    frame.extend_from_slice(b"VP8 ");
    frame.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    frame.extend_from_slice(&payload);
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
    assert_eq!(animation.frame_count(), 1);
    assert_eq!(animation.frame(1), None);
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
            bitstream: FrameBitstream::Vp8(&payload)
        }]
    );
}

#[test]
fn frame_layout_and_resource_limits_are_rejected() {
    let vp8x = [0b0000_0010, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    let mut out_of_bounds = vec![1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    let payload = vp8(1, 1);
    out_of_bounds.extend_from_slice(b"VP8 ");
    out_of_bounds.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    out_of_bounds.extend_from_slice(&payload);
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
    frame.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    frame.extend_from_slice(&payload);
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
    let payload = vp8(1, 1);
    frame.extend_from_slice(b"VP8 ");
    frame.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    frame.extend_from_slice(&payload);
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

#[test]
fn frame_bitstream_dimensions_and_nested_chunk_budget_are_enforced() {
    let vp8x = [0b0000_0010, 0, 0, 0, 1, 0, 0, 0, 0, 0];
    let payload = vp8(1, 1);
    let mut frame = vec![0; 16];
    frame[6] = 1; // ANMF width is 2, but the bitstream width is 1.
    frame.extend_from_slice(b"VP8 ");
    frame.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    frame.extend_from_slice(&payload);
    let bytes = riff(&[
        (VP8X, &vp8x, None),
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
        ContainerErrorKind::InvalidDimensions
    );

    frame[6] = 0;
    for fourcc in [*b"u001", *b"u002", *b"u003"] {
        frame.extend_from_slice(&fourcc);
        frame.extend_from_slice(&0_u32.to_le_bytes());
    }
    let bytes = riff(&[
        (VP8X, &vp8x, None),
        (ANIM, &[0; 6], None),
        (ANMF, &frame, None),
    ]);
    let limits = ContainerLimits {
        max_chunks: 3,
        ..ContainerLimits::default()
    };
    assert_eq!(
        parse(&bytes, CompatibilityProfile::SpecStrict, &limits)
            .unwrap_err()
            .kind(),
        ContainerErrorKind::LimitExceeded
    );
}
