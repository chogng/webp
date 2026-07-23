use super::*;
use crate::vp8l::EntropyToken;

#[test]
fn token_start_owns_a_copy_that_crosses_a_block_boundary() {
    let rgba = [1, 2, 3, 255].repeat(300);
    let stream = TokenStream::collect(&rgba, 300, false, false, 0)
        .expect("collect boundary-crossing stream");
    assert_eq!(
        stream.tokens(),
        [
            EntropyToken::Literal([1, 2, 3, 255]),
            EntropyToken::Copy { length: 299 }
        ]
    );
    let plan = SpatialPlan::build(&stream, SpatialProfile::Compact).expect("build spatial plan");
    assert_eq!(plan.group_for_pixel(1), usize::from(plan.group_map()[0]));
    assert_eq!(plan.group_map().len(), 3);
}
