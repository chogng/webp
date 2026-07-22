use super::prefix_image_dimensions;

#[test]
fn meta_prefix_dimensions_round_up_for_every_prefix_bits_value() {
    for field in 0..=7_u8 {
        let bits = field + 2;
        let (width, height) = prefix_image_dimensions(513, 1025, bits).unwrap();
        let block = 1_u32 << bits;
        assert_eq!(width, 513_u32.div_ceil(block));
        assert_eq!(height, 1025_u32.div_ceil(block));
    }
}
