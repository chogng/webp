//! Bounded byte adapters for VP8L fuzz targets.

use crate::BitReader;
use crate::vp8l::huffman::read_huffman_code;
use crate::vp8l::transforms::color::ColorTransform;
use crate::vp8l::transforms::color::ColorTransformMultipliers;
use crate::vp8l::transforms::color::inverse_color_transform;
use crate::vp8l::transforms::indexing::Palette;
use crate::vp8l::transforms::indexing::inverse_color_indexing;
use crate::vp8l::transforms::predictor::PredictorMode;
use crate::vp8l::transforms::predictor::Rgba;
use crate::vp8l::transforms::predictor::RgbaImage;
use crate::vp8l::transforms::predictor::inverse_predictor;
use crate::vp8l::transforms::predictor::inverse_subtract_green;

pub(crate) fn huffman(input: &[u8]) {
    let Some((&low, rest)) = input.split_first() else {
        return;
    };
    let Some((&high, encoded_code)) = rest.split_first() else {
        return;
    };
    let requested = usize::from(u16::from_le_bytes([low, high]));
    let mut bits = BitReader::new(encoded_code);
    let _ = read_huffman_code(&mut bits, requested % 2_328 + 1);
}

fn byte_at(input: &[u8], index: usize) -> u8 {
    input[index % input.len()]
}

pub(crate) fn transforms(input: &[u8]) {
    if input.len() < 2 {
        return;
    }
    let width = u32::from(input[0] % 16) + 1;
    let height = u32::from(input[1] % 16) + 1;
    let pixel_count = usize::try_from(width * height).expect("bounded dimensions");
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
                .expect("bounded mode")
        })
        .collect::<Vec<_>>();
    let mut image = RgbaImage::new(width, height, pixels).expect("exact image");
    inverse_predictor(&mut image, &modes).expect("matching modes");
    inverse_subtract_green(&mut image);

    let block_bits = input[0] % 4;
    let block_size = 1_u32 << block_bits;
    let blocks_wide = width.div_ceil(block_size);
    let blocks_high = height.div_ceil(block_size);
    let block_count = usize::try_from(blocks_wide * blocks_high).expect("bounded blocks");
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
        .expect("exact transform");
    inverse_color_transform(&mut image, &transform).expect("covering transform");

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
    .expect("bounded palette");
    let packed_width = palette
        .packing()
        .packed_width(width)
        .expect("bounded width");
    let packed_count = usize::try_from(packed_width * height).expect("bounded image");
    let indexed = RgbaImage::new(
        packed_width,
        height,
        (0..packed_count)
            .map(|index| Rgba::new(0, byte_at(input, 2 + index), 0, 0))
            .collect(),
    )
    .expect("exact indexed image");
    let _ = inverse_color_indexing(&palette, width, &indexed).expect("matching packing");
}
