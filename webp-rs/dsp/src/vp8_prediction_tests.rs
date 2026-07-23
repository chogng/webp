//! Tests for VP8 prediction and sample-reconstruction kernels.

use super::*;
#[test]
fn intra16_prediction_uses_neighbours_and_boundary_fallbacks() {
    let edges = MacroblockPredictionEdges {
        top_y: Some([10; 16]),
        top_right_y: Some([10; 4]),
        left_y: Some([30; 16]),
        top_left_y: 5,
        top_u: Some([50; 8]),
        left_u: Some([70; 8]),
        top_left_u: 20,
        top_v: Some([80; 8]),
        left_v: Some([90; 8]),
        top_left_v: 30,
    };
    let prediction =
        predict_intra16_macroblock(Intra16Mode::Vertical, ChromaMode::Horizontal, edges);
    assert_eq!(prediction.y, [10; 256]);
    assert_eq!(prediction.u, [70; 64]);
    assert_eq!(prediction.v, [90; 64]);

    let true_motion =
        predict_intra16_macroblock(Intra16Mode::TrueMotion, ChromaMode::TrueMotion, edges);
    assert_eq!(true_motion.y, [35; 256]);
    assert_eq!(true_motion.u, [100; 64]);
    assert_eq!(true_motion.v, [140; 64]);

    let dc = predict_intra16_macroblock(
        Intra16Mode::Dc,
        ChromaMode::Dc,
        MacroblockPredictionEdges::default(),
    );
    assert_eq!(dc.y, [128; 256]);
    assert_eq!(dc.u, [128; 64]);
    assert_eq!(dc.v, [128; 64]);
}

#[test]
fn intra4_prediction_covers_all_directional_modes() {
    let top = [10, 20, 30, 40, 50, 60, 70, 80];
    let left = [50, 60, 70, 80];
    assert_eq!(predict_intra4_block(Intra4Mode::Dc, 5, top, left), [45; 16]);
    let true_motion = predict_intra4_block(Intra4Mode::TrueMotion, 5, top, left);
    assert_eq!((true_motion[0], true_motion[15]), (55, 115));
    assert_eq!(
        predict_intra4_block(Intra4Mode::Vertical, 5, top, left),
        [
            11, 20, 30, 40, 11, 20, 30, 40, 11, 20, 30, 40, 11, 20, 30, 40
        ]
    );
    assert_eq!(
        predict_intra4_block(Intra4Mode::Horizontal, 5, top, left),
        [
            41, 41, 41, 41, 60, 60, 60, 60, 70, 70, 70, 70, 78, 78, 78, 78
        ]
    );
    for (mode, expected) in [
        (
            Intra4Mode::DiagonalDownRight,
            [
                18, 11, 20, 30, 41, 18, 11, 20, 60, 41, 18, 11, 70, 60, 41, 18,
            ],
        ),
        (
            Intra4Mode::VerticalRight,
            [8, 15, 25, 35, 18, 11, 20, 30, 41, 8, 15, 25, 60, 18, 11, 20],
        ),
        (
            Intra4Mode::DiagonalDownLeft,
            [
                20, 30, 40, 50, 30, 40, 50, 60, 40, 50, 60, 70, 50, 60, 70, 78,
            ],
        ),
        (
            Intra4Mode::VerticalLeft,
            [
                15, 25, 35, 45, 20, 30, 40, 50, 25, 35, 45, 60, 30, 40, 50, 70,
            ],
        ),
        (
            Intra4Mode::HorizontalDown,
            [
                28, 18, 11, 20, 55, 41, 28, 18, 65, 60, 55, 41, 75, 70, 65, 60,
            ],
        ),
        (
            Intra4Mode::HorizontalUp,
            [
                55, 60, 65, 70, 65, 70, 75, 78, 75, 78, 80, 80, 80, 80, 80, 80,
            ],
        ),
    ] {
        assert_eq!(predict_intra4_block(mode, 5, top, left), expected);
    }
}
