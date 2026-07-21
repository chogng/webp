#![no_main]
#![forbid(unsafe_code)]

use libfuzzer_sys::fuzz_target;
use webp_vp8l_color_transform::ColorTransform;
use webp_vp8l_color_transform::ColorTransformMultipliers;
use webp_vp8l_color_transform::Rgba as ColorRgba;
use webp_vp8l_color_transform::RgbaImage as ColorImage;
use webp_vp8l_color_transform::inverse_color_transform;
use webp_vp8l_indexing::Palette;
use webp_vp8l_indexing::inverse_color_indexing;
use webp_vp8l_transform::PredictorMode;
use webp_vp8l_transform::Rgba;
use webp_vp8l_transform::RgbaImage;
use webp_vp8l_transform::inverse_predictor;
use webp_vp8l_transform::inverse_subtract_green;

const MAX_DIMENSION: u32 = 16;

fn byte_at(input: &[u8], index: usize) -> u8 {
    input.get(index % input.len()).copied().unwrap_or(0)
}

fuzz_target!(|input: &[u8]| {
    if input.len() < 2 {
        return;
    }
    let width = u32::from(input[0] % MAX_DIMENSION as u8) + 1;
    let height = u32::from(input[1] % MAX_DIMENSION as u8) + 1;
    let pixel_count = usize::try_from(width * height).expect("bounded dimensions fit usize");

    let pixels = (0..pixel_count)
        .map(|index| {
            let base = 2 + index * 4;
            Rgba::new(
                byte_at(input, base),
                byte_at(input, base + 1),
                byte_at(input, base + 2),
                byte_at(input, base + 3),
            )
        })
        .collect();
    let modes = (0..pixel_count)
        .map(|index| {
            PredictorMode::try_from(byte_at(input, 2 + pixel_count * 4 + index) % 14)
                .expect("modulo produces a valid VP8L predictor mode")
        })
        .collect::<Vec<_>>();
    let mut image = RgbaImage::new(width, height, pixels).expect("bounded pixel buffer is exact");
    inverse_predictor(&mut image, &modes).expect("mode buffer matches image");
    inverse_subtract_green(&mut image);

    let block_bits = input[0] % 4;
    let block_size = 1_u32 << block_bits;
    let blocks_wide = width.div_ceil(block_size);
    let blocks_high = height.div_ceil(block_size);
    let block_count =
        usize::try_from(blocks_wide * blocks_high).expect("bounded transform dimensions fit usize");
    let multipliers = (0..block_count)
        .map(|index| {
            let base = 2 + index * 3;
            ColorTransformMultipliers::new(
                byte_at(input, base) as i8,
                byte_at(input, base + 1) as i8,
                byte_at(input, base + 2) as i8,
            )
        })
        .collect();
    let transform = ColorTransform::new(block_bits, blocks_wide, blocks_high, multipliers)
        .expect("bounded block table is exact");
    let color_pixels = image
        .pixels()
        .iter()
        .map(|pixel| ColorRgba::new(pixel.red, pixel.green, pixel.blue, pixel.alpha))
        .collect();
    let mut color_image =
        ColorImage::new(width, height, color_pixels).expect("bounded color buffer is exact");
    inverse_color_transform(&mut color_image, &transform)
        .expect("transform covers every image coordinate");

    let palette_size = usize::from(input[1]) + 1;
    let palette = Palette::new(
        (0..palette_size)
            .map(|index| {
                let base = 2 + index * 4;
                Rgba::new(
                    byte_at(input, base),
                    byte_at(input, base + 1),
                    byte_at(input, base + 2),
                    byte_at(input, base + 3),
                )
            })
            .collect(),
    )
    .expect("palette size is in the VP8L range");
    let packed_width = palette
        .packing()
        .packed_width(width)
        .expect("bounded output width cannot overflow");
    let packed_count =
        usize::try_from(packed_width * height).expect("bounded indexed dimensions fit usize");
    let indexed_pixels = (0..packed_count)
        .map(|index| Rgba::new(0, byte_at(input, 2 + index), 0, 0))
        .collect();
    let indexed =
        RgbaImage::new(packed_width, height, indexed_pixels).expect("indexed buffer is exact");
    let _ = inverse_color_indexing(&palette, width, &indexed)
        .expect("packing and indexed dimensions agree");
});
