use super::inverse_subtract_green_argb;
use crate::pixel::pack_argb;
use crate::pixel::unpack_rgba;

#[test]
fn inverse_subtract_green_preserves_green_and_alpha_for_each_pixel() {
    let mut pixels = [
        pack_argb(0xf0, 0x30, 0xee, 0x80),
        pack_argb(0x01, 0xff, 0x02, 0x7f),
    ];
    inverse_subtract_green_argb(&mut pixels);
    assert_eq!(unpack_rgba(pixels[0]), [0x20, 0x30, 0x1e, 0x80]);
    assert_eq!(unpack_rgba(pixels[1]), [0x00, 0xff, 0x01, 0x7f]);
}
