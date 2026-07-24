use super::{
    inverse_color_argb, inverse_color_rgba, inverse_subtract_green_argb,
    inverse_subtract_green_rgba,
};
use crate::vp8l::header::BlockTransformDescriptor;
use crate::vp8l::pixel::pack_argb;
use crate::vp8l::pixel::unpack_rgba;
use crate::vp8l::transforms::color::ColorTransformMultipliers;

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

#[test]
fn rgba_subtract_green_matches_packed_argb() {
    let mut packed = [pack_argb(250, 10, 3, 7), pack_argb(1, 255, 128, 200)];
    let mut rgba = packed
        .iter()
        .flat_map(|&pixel| crate::vp8l::pixel::unpack_rgba(pixel))
        .collect::<Vec<_>>();
    inverse_subtract_green_argb(&mut packed);
    inverse_subtract_green_rgba(&mut rgba);
    let expected = packed
        .iter()
        .flat_map(|&pixel| crate::vp8l::pixel::unpack_rgba(pixel))
        .collect::<Vec<_>>();
    assert_eq!(rgba, expected);
}

#[test]
fn rgba_color_transform_matches_packed_argb_across_partial_blocks() {
    let descriptor = BlockTransformDescriptor {
        image_width: 5,
        image_height: 3,
        block_size_bits: 2,
        transform_width: 2,
        transform_height: 1,
    };
    let multipliers = [
        ColorTransformMultipliers::new(-32, 17, 63),
        ColorTransformMultipliers::new(127, -128, -1),
    ];
    let mut packed = (0_u8..15)
        .map(|value| {
            pack_argb(
                value.wrapping_mul(29),
                value.wrapping_mul(47),
                value.wrapping_mul(71),
                value.wrapping_mul(13),
            )
        })
        .collect::<Vec<_>>();
    let mut rgba = packed
        .iter()
        .flat_map(|&pixel| unpack_rgba(pixel))
        .collect::<Vec<_>>();

    inverse_color_argb(&mut packed, descriptor, &multipliers).unwrap();
    inverse_color_rgba(&mut rgba, descriptor, &multipliers).unwrap();

    let expected = packed
        .iter()
        .flat_map(|&pixel| unpack_rgba(pixel))
        .collect::<Vec<_>>();
    assert_eq!(rgba, expected);
}
