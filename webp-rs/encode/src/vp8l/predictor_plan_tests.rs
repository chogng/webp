use super::*;

#[test]
fn adaptive_plan_is_bounded_and_covers_every_block() {
    let width = 33;
    let height = 17;
    let mut rgba = Vec::new();
    for y in 0..height {
        for x in 0..width {
            rgba.extend_from_slice(&[x as u8, y as u8, (x + y) as u8, 255]);
        }
    }
    let plan = PredictorPlan::adaptive(&rgba, width, true, None).expect("build predictor plan");
    let PredictorPlan::Blocks {
        block_width, modes, ..
    } = plan
    else {
        panic!("adaptive plan must own block modes");
    };
    assert_eq!(block_width, width.div_ceil(16));
    assert_eq!(modes.len(), width.div_ceil(16) * height.div_ceil(16));
    assert!(modes.iter().all(|mode| CANDIDATE_MODES.contains(mode)));
}
