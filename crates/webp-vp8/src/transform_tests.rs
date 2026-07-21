use super::*;

#[test]
fn inverse_dct_preserves_zero_and_dc_microvectors() {
    assert_eq!(inverse_dct_4x4([0; 16]), [0; 16]);
    let mut dc = [0_i16; 16];
    dc[0] = 16;
    assert_eq!(inverse_dct_4x4(dc), [2; 16]);
}

#[test]
fn inverse_wht_distributes_y2_dc_to_all_macroblock_blocks() {
    assert_eq!(inverse_wht_4x4([0; 16]), [0; 16]);
    let mut dc = [0_i16; 16];
    dc[0] = 8;
    assert_eq!(inverse_wht_4x4(dc), [1; 16]);
}
