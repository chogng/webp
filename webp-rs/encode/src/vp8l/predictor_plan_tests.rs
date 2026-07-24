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

#[test]
fn adaptive_plan_supports_large_bounded_blocks() {
    let rgba = [11, 23, 37, 255].repeat(17 * 19);
    let plan = PredictorPlan::adaptive_with_block_bits(&rgba, 17, true, None, 9)
        .expect("build large predictor plan");
    assert_eq!(plan.block_bits(), 9);
    assert!(plan.mode_at(17 * 18, 17).is_some());
    assert_eq!(
        PredictorPlan::adaptive_with_block_bits(&rgba, 17, true, None, 10).err(),
        Some(EncodeError::SizeOverflow)
    );
}
