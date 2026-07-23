use super::*;
use crate::vp8l::TokenStream;

#[test]
fn full_256_pixel_block_does_not_overflow_histogram_counters() {
    let mut rgba = Vec::with_capacity(256 * 256 * 4);
    for index in 0..256 * 256 {
        rgba.extend_from_slice(&[1, index as u8, 3, 255]);
    }
    let stream = TokenStream::collect_for_spatial(&rgba, 256, false, false, 0, 256)
        .expect("collect full literal block");
    let clustered = cluster_tokens(stream.spatial_blocks(256).expect("block facts"), 16)
        .expect("cluster full block");
    assert_eq!(clustered.assignments.len(), 1);
    assert_eq!(clustered.group_count, 1);
}
