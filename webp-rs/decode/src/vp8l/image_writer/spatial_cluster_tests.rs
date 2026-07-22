use super::*;

#[test]
fn full_256_pixel_block_does_not_overflow_histogram_counters() {
    let tokens = vec![EntropyToken::Literal([1, 2, 3, 255]); 256 * 256];
    let clustered = cluster_tokens(&tokens, 256, 256, 256, 16).expect("cluster full block");
    assert_eq!(clustered.assignments.len(), 1);
    assert_eq!(clustered.group_count, 1);
}
