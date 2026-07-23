//! Tests for fixed VP8 and VP8L image headers.

use super::*;

#[test]
fn parses_vp8_and_vp8l_dimensions_and_alpha_hint() {
    let vp8 = [0x10, 0, 0, 0x9d, 0x01, 0x2a, 7, 0, 9, 0];
    assert_eq!(
        parse(VP8, &vp8, &ContainerLimits::default(), 20).unwrap(),
        ImageHeader {
            width: 7,
            height: 9,
            alpha_hint: false,
        }
    );

    let fields = (4_u32 - 1) | ((6_u32 - 1) << 14) | (1 << 28);
    let mut vp8l = vec![VP8L_SIGNATURE];
    vp8l.extend_from_slice(&fields.to_le_bytes());
    assert_eq!(
        parse(VP8L, &vp8l, &ContainerLimits::default(), 40).unwrap(),
        ImageHeader {
            width: 4,
            height: 6,
            alpha_hint: true,
        }
    );
}

#[test]
fn rejects_truncated_invalid_and_over_limit_headers() {
    assert_eq!(
        parse(VP8, &[0; 9], &ContainerLimits::default(), 0)
            .unwrap_err()
            .kind(),
        ContainerErrorKind::UnexpectedEof
    );
    assert_eq!(
        parse(VP8L, &[0; 5], &ContainerLimits::default(), 0)
            .unwrap_err()
            .kind(),
        ContainerErrorKind::InvalidContainer
    );
    let limits = ContainerLimits {
        max_width: 3,
        ..ContainerLimits::default()
    };
    let fields = 3_u32;
    let mut vp8l = vec![VP8L_SIGNATURE];
    vp8l.extend_from_slice(&fields.to_le_bytes());
    assert_eq!(
        parse(VP8L, &vp8l, &limits, 0).unwrap_err().kind(),
        ContainerErrorKind::LimitExceeded
    );
}
