use super::*;

fn limits() -> DecodeLimits {
    DecodeLimits::default()
}

#[test]
fn replace_blend_and_disposal_follow_frame_order() {
    let mut canvas = AnimationCanvas::new(2, 1, [4, 3, 2, 1], &limits()).unwrap();
    canvas
        .compose(
            DecodedFrame {
                x: 0,
                y: 0,
                width: 1,
                height: 1,
                rgba: &[10, 20, 30, 255],
                blend: false,
                dispose_to_background: true,
            },
            &limits(),
        )
        .unwrap();
    assert_eq!(canvas.rgba(), &[10, 20, 30, 255, 0, 0, 0, 0]);
    canvas
        .compose(
            DecodedFrame {
                x: 1,
                y: 0,
                width: 1,
                height: 1,
                rgba: &[100, 0, 0, 255],
                blend: false,
                dispose_to_background: false,
            },
            &limits(),
        )
        .unwrap();
    assert_eq!(canvas.rgba(), &[0, 0, 0, 0, 100, 0, 0, 255]);
}

#[test]
fn alpha_blend_keeps_straight_rgba() {
    let mut canvas = AnimationCanvas::new(1, 1, [0, 0, 0, 0], &limits()).unwrap();
    canvas
        .compose(
            DecodedFrame {
                x: 0,
                y: 0,
                width: 1,
                height: 1,
                rgba: &[0, 0, 255, 255],
                blend: false,
                dispose_to_background: false,
            },
            &limits(),
        )
        .unwrap();
    canvas
        .compose(
            DecodedFrame {
                x: 0,
                y: 0,
                width: 1,
                height: 1,
                rgba: &[255, 0, 0, 128],
                blend: true,
                dispose_to_background: false,
            },
            &limits(),
        )
        .unwrap();
    assert_eq!(canvas.rgba(), &[128, 0, 127, 255]);
}

#[test]
fn full_canvas_replace_skips_an_obsolete_pending_disposal() {
    let mut canvas = AnimationCanvas::new(1, 1, [4, 3, 2, 1], &limits()).unwrap();
    canvas
        .compose(
            DecodedFrame {
                x: 0,
                y: 0,
                width: 1,
                height: 1,
                rgba: &[10, 20, 30, 255],
                blend: false,
                dispose_to_background: true,
            },
            &limits(),
        )
        .unwrap();
    let limited = DecodeLimits {
        max_work_units: 1,
        ..limits()
    };
    canvas
        .compose(
            DecodedFrame {
                x: 0,
                y: 0,
                width: 1,
                height: 1,
                rgba: &[100, 110, 120, 255],
                blend: false,
                dispose_to_background: false,
            },
            &limited,
        )
        .unwrap();
    assert_eq!(canvas.rgba(), &[100, 110, 120, 255]);
}

#[test]
fn invalid_rectangles_lengths_and_work_are_rejected() {
    let mut canvas = AnimationCanvas::new(1, 1, [0; 4], &limits()).unwrap();
    assert_eq!(
        canvas
            .compose(
                DecodedFrame {
                    x: 1,
                    y: 0,
                    width: 1,
                    height: 1,
                    rgba: &[0; 4],
                    blend: false,
                    dispose_to_background: false
                },
                &limits()
            )
            .unwrap_err()
            .kind(),
        DecodeErrorKind::InvalidParameter
    );
    assert_eq!(
        canvas
            .compose(
                DecodedFrame {
                    x: 0,
                    y: 0,
                    width: 1,
                    height: 1,
                    rgba: &[],
                    blend: false,
                    dispose_to_background: false
                },
                &limits()
            )
            .unwrap_err()
            .kind(),
        DecodeErrorKind::InvalidParameter
    );
    let limited = DecodeLimits {
        max_work_units: 0,
        ..limits()
    };
    assert_eq!(
        canvas
            .compose(
                DecodedFrame {
                    x: 0,
                    y: 0,
                    width: 1,
                    height: 1,
                    rgba: &[0; 4],
                    blend: false,
                    dispose_to_background: false
                },
                &limited
            )
            .unwrap_err()
            .kind(),
        DecodeErrorKind::LimitExceeded
    );
}
