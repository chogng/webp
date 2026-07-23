use crate::vp8l::transforms::predictor::Rgba;

pub(super) const fn pack_argb(red: u8, green: u8, blue: u8, alpha: u8) -> u32 {
    ((alpha as u32) << 24) | ((red as u32) << 16) | ((green as u32) << 8) | (blue as u32)
}

pub(super) const fn argb_to_rgba(pixel: u32) -> Rgba {
    Rgba::new(
        (pixel >> 16) as u8,
        (pixel >> 8) as u8,
        pixel as u8,
        (pixel >> 24) as u8,
    )
}

pub(super) const fn unpack_rgba(pixel: u32) -> [u8; 4] {
    [
        (pixel >> 16) as u8,
        (pixel >> 8) as u8,
        pixel as u8,
        (pixel >> 24) as u8,
    ]
}

#[cfg(test)]
pub(super) fn extend_rgba_from_argb(output: &mut Vec<u8>, pixels: &[u32]) {
    let mut blocks = pixels.chunks_exact(8);
    for block in &mut blocks {
        let first = unpack_rgba(block[0]);
        let second = unpack_rgba(block[1]);
        let third = unpack_rgba(block[2]);
        let fourth = unpack_rgba(block[3]);
        let fifth = unpack_rgba(block[4]);
        let sixth = unpack_rgba(block[5]);
        let seventh = unpack_rgba(block[6]);
        let eighth = unpack_rgba(block[7]);
        output.extend_from_slice(&[
            first[0], first[1], first[2], first[3], second[0], second[1], second[2], second[3],
            third[0], third[1], third[2], third[3], fourth[0], fourth[1], fourth[2], fourth[3],
            fifth[0], fifth[1], fifth[2], fifth[3], sixth[0], sixth[1], sixth[2], sixth[3],
            seventh[0], seventh[1], seventh[2], seventh[3], eighth[0], eighth[1], eighth[2],
            eighth[3],
        ]);
    }
    let mut tail_blocks = blocks.remainder().chunks_exact(4);
    for block in &mut tail_blocks {
        let first = unpack_rgba(block[0]);
        let second = unpack_rgba(block[1]);
        let third = unpack_rgba(block[2]);
        let fourth = unpack_rgba(block[3]);
        output.extend_from_slice(&[
            first[0], first[1], first[2], first[3], second[0], second[1], second[2], second[3],
            third[0], third[1], third[2], third[3], fourth[0], fourth[1], fourth[2], fourth[3],
        ]);
    }
    for &pixel in tail_blocks.remainder() {
        output.extend_from_slice(&unpack_rgba(pixel));
    }
}
