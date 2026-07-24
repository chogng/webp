use super::inverse_color_indexing_rgba;
use crate::vp8l::header::ColorIndexingDescriptor;
use crate::vp8l::transforms::indexing::Palette;
use crate::vp8l::transforms::predictor::Rgba;

#[test]
fn palette_expansion_reuses_the_preallocated_backing_across_rows() {
    let palette = Palette::new(vec![Rgba::new(1, 2, 3, 4), Rgba::new(10, 20, 30, 40)]).unwrap();
    let descriptor = ColorIndexingDescriptor {
        image_width_before: 10,
        image_height: 2,
        color_table_size: 2,
        width_bits: 3,
        image_width_after: 2,
    };
    let mut pixels = Vec::with_capacity(80);
    pixels.extend_from_slice(&[
        0,
        0b1010_0101,
        0,
        0,
        0,
        0b0000_0010,
        0,
        0,
        0,
        0b0101_1010,
        0,
        0,
        0,
        0b0000_0001,
        0,
        0,
    ]);
    let original_address = pixels.as_ptr();
    let original_capacity = pixels.capacity();

    inverse_color_indexing_rgba(&mut pixels, descriptor, &palette, 0, 80, 176).unwrap();

    assert_eq!(pixels.as_ptr(), original_address);
    assert_eq!(pixels.capacity(), original_capacity);
    assert_eq!(pixels.len(), 80);
    let actual = pixels
        .chunks_exact(4)
        .map(|pixel| pixel[0])
        .collect::<Vec<_>>();
    assert_eq!(
        actual,
        [
            10, 1, 10, 1, 1, 10, 1, 10, 1, 10, 1, 10, 1, 10, 10, 1, 10, 1, 10, 1
        ]
    );
}
