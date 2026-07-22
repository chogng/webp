use super::decode_literal_only;
use super::decode_no_transform;
use crate::BitWriter;
use crate::DecodeErrorKind;
use crate::DecodeLimits;
use crate::vp8l::header::HEADER_LEN;
use crate::vp8l::header::SIGNATURE;
use crate::vp8l::image_stream::huffman_groups::CHANNEL_ALPHABET_SIZE;
use crate::vp8l::image_stream::huffman_groups::GREEN_ALPHABET_SIZE;
use crate::vp8l::image_stream::symbol_stream::prefix_image_dimensions;
use crate::vp8l::pixel::pack_argb;

fn limits() -> DecodeLimits {
    DecodeLimits::default()
}

fn write_header(writer: &mut BitWriter, width: u32, height: u32, alpha: bool) {
    writer.write_bits(u32::from(SIGNATURE), 8).unwrap();
    writer.write_bits(width - 1, 14).unwrap();
    writer.write_bits(height - 1, 14).unwrap();
    writer.write_bits(u32::from(alpha), 1).unwrap();
    writer.write_bits(0, 3).unwrap();
}

fn write_simple_code(writer: &mut BitWriter, symbol: u8) {
    writer.write_bits(1, 1).unwrap(); // simple_code_flag
    writer.write_bits(0, 1).unwrap(); // one symbol
    writer.write_bits(1, 1).unwrap(); // first symbol uses eight bits
    writer.write_bits(u32::from(symbol), 8).unwrap();
}

fn write_two_symbol_normal_code(
    writer: &mut BitWriter,
    alphabet_size: usize,
    first_symbol: usize,
    second_symbol: usize,
) {
    assert!(first_symbol < second_symbol);
    assert!(second_symbol < alphabet_size);
    writer.write_bits(0, 1).unwrap(); // normal_code_flag
    writer.write_bits(0, 4).unwrap(); // four code-length alphabet entries

    // Wire order is 17, 18, 0, 1. Symbols zero and one form a complete
    // code-length tree, so the following code lengths use one bit each.
    for length in [0_u32, 0, 1, 1] {
        writer.write_bits(length, 3).unwrap();
    }
    writer.write_bits(0, 1).unwrap(); // use_length = false
    for symbol in 0..alphabet_size {
        writer
            .write_bits(
                u32::from(symbol == first_symbol || symbol == second_symbol),
                1,
            )
            .unwrap();
    }
}

fn wire_code(lengths: &[u8], wanted_symbol: usize) -> (u32, u8) {
    let mut sorted: Vec<(u8, usize)> = lengths
        .iter()
        .copied()
        .enumerate()
        .filter_map(|(symbol, length)| (length != 0).then_some((length, symbol)))
        .collect();
    sorted.sort_unstable();

    let mut code = 0_u32;
    let mut previous_length = 0_u8;
    for (length, symbol) in sorted {
        code <<= u32::from(length - previous_length);
        if symbol == wanted_symbol {
            return (
                code.reverse_bits() >> (u32::BITS - u32::from(length)),
                length,
            );
        }
        code += 1;
        previous_length = length;
    }
    panic!("requested unused Huffman symbol");
}

fn write_normal_code(
    writer: &mut BitWriter,
    alphabet_size: usize,
    entries: &[(usize, u8)],
) -> Vec<u8> {
    let mut lengths = vec![0_u8; alphabet_size];
    for &(symbol, length) in entries {
        assert!(symbol < alphabet_size);
        lengths[symbol] = length;
    }

    writer.write_bits(0, 1).unwrap(); // normal_code_flag

    // Code-length symbols 0, 1, 2 and 3 all have two-bit codes. This
    // lets the fixture express the small complete trees used below.
    writer.write_bits(2, 4).unwrap(); // 4 + 2 == 6 entries
    for length in [0_u32, 0, 2, 2, 2, 2] {
        writer.write_bits(length, 3).unwrap();
    }
    writer.write_bits(0, 1).unwrap(); // use_length = false
    let code_length_lengths = [2_u8; 4];
    for &length in &lengths {
        let (code, width) = wire_code(&code_length_lengths, usize::from(length));
        writer.write_bits(code, width).unwrap();
    }
    lengths
}

fn write_symbol(writer: &mut BitWriter, lengths: &[u8], symbol: usize) {
    let (code, width) = wire_code(lengths, symbol);
    writer.write_bits(code, width).unwrap();
}

fn literal_stream(width: u32, height: u32, pixel: [u8; 4]) -> Vec<u8> {
    literal_stream_with_transforms(width, height, pixel, &[])
}

fn literal_stream_with_transforms(
    width: u32,
    height: u32,
    pixel: [u8; 4],
    transforms: &[u8],
) -> Vec<u8> {
    let mut writer = BitWriter::new();
    write_header(&mut writer, width, height, pixel[3] != 255);
    for &transform_type in transforms {
        writer.write_bits(1, 1).unwrap(); // transform_present
        writer.write_bits(u32::from(transform_type), 2).unwrap();
    }
    writer.write_bits(0, 1).unwrap(); // transform list terminator
    writer.write_bits(0, 1).unwrap(); // color_cache_present
    writer.write_bits(0, 1).unwrap(); // use_meta_huffman
    for symbol in [pixel[1], pixel[0], pixel[2], pixel[3], 0] {
        write_simple_code(&mut writer, symbol);
    }
    writer.into_bytes()
}

fn write_flat_entropy_image(writer: &mut BitWriter, pixel: [u8; 4], is_level0: bool) {
    writer.write_bits(0, 1).unwrap(); // color_cache_present
    if is_level0 {
        writer.write_bits(0, 1).unwrap(); // use_meta_huffman
    }
    for symbol in [pixel[1], pixel[0], pixel[2], pixel[3], 0] {
        write_simple_code(writer, symbol);
    }
}

fn write_channel_code(
    writer: &mut BitWriter,
    alphabet_size: usize,
    values: &[u8],
) -> Option<Vec<u8>> {
    let mut symbols = values.to_vec();
    symbols.sort_unstable();
    symbols.dedup();
    match symbols.len() {
        1 => {
            write_simple_code(writer, symbols[0]);
            None
        }
        2 => Some(write_normal_code(
            writer,
            alphabet_size,
            &[(usize::from(symbols[0]), 1), (usize::from(symbols[1]), 1)],
        )),
        3 => Some(write_normal_code(
            writer,
            alphabet_size,
            &[
                (usize::from(symbols[0]), 1),
                (usize::from(symbols[1]), 2),
                (usize::from(symbols[2]), 2),
            ],
        )),
        4 => Some(write_normal_code(
            writer,
            alphabet_size,
            &[
                (usize::from(symbols[0]), 2),
                (usize::from(symbols[1]), 2),
                (usize::from(symbols[2]), 2),
                (usize::from(symbols[3]), 2),
            ],
        )),
        _ => panic!("test helper supports at most four channel symbols"),
    }
}

/// Writes a small non-level-zero VP8L entropy image with literal pixels.
fn write_entropy_image_pixels(writer: &mut BitWriter, pixels: &[[u8; 4]]) {
    write_entropy_image_pixels_at_level(writer, pixels, false);
}

fn write_entropy_image_pixels_at_level(
    writer: &mut BitWriter,
    pixels: &[[u8; 4]],
    is_level0: bool,
) {
    assert!(!pixels.is_empty());
    writer.write_bits(0, 1).unwrap(); // color_cache_present
    if is_level0 {
        writer.write_bits(0, 1).unwrap(); // use_meta_huffman
    }
    let green = write_channel_code(
        writer,
        GREEN_ALPHABET_SIZE,
        &pixels.iter().map(|pixel| pixel[1]).collect::<Vec<_>>(),
    );
    let red = write_channel_code(
        writer,
        CHANNEL_ALPHABET_SIZE,
        &pixels.iter().map(|pixel| pixel[0]).collect::<Vec<_>>(),
    );
    let blue = write_channel_code(
        writer,
        CHANNEL_ALPHABET_SIZE,
        &pixels.iter().map(|pixel| pixel[2]).collect::<Vec<_>>(),
    );
    let alpha = write_channel_code(
        writer,
        CHANNEL_ALPHABET_SIZE,
        &pixels.iter().map(|pixel| pixel[3]).collect::<Vec<_>>(),
    );
    write_simple_code(writer, 0); // distance prefix

    for pixel in pixels {
        if let Some(lengths) = &green {
            write_symbol(writer, lengths, usize::from(pixel[1]));
        }
        if let Some(lengths) = &red {
            write_symbol(writer, lengths, usize::from(pixel[0]));
        }
        if let Some(lengths) = &blue {
            write_symbol(writer, lengths, usize::from(pixel[2]));
        }
        if let Some(lengths) = &alpha {
            write_symbol(writer, lengths, usize::from(pixel[3]));
        }
    }
}

fn meta_huffman_literal_stream(
    width: u32,
    height: u32,
    prefix_bits_field: u8,
    group_map: &[u16],
    group_pixels: &[[u8; 4]],
) -> Vec<u8> {
    let prefix_bits = prefix_bits_field + 2;
    let (map_width, map_height) = prefix_image_dimensions(width, height, prefix_bits).unwrap();
    assert_eq!(
        group_map.len(),
        usize::try_from(map_width * map_height).unwrap()
    );
    let max_group = usize::from(*group_map.iter().max().unwrap());
    assert_eq!(group_pixels.len(), max_group + 1);

    let mut writer = BitWriter::new();
    write_header(&mut writer, width, height, true);
    writer.write_bits(0, 1).unwrap(); // transform-list terminator
    writer.write_bits(0, 1).unwrap(); // color_cache_present
    writer.write_bits(1, 1).unwrap(); // use_meta_huffman
    writer.write_bits(u32::from(prefix_bits_field), 3).unwrap();
    let entropy_pixels: Vec<[u8; 4]> = group_map
        .iter()
        .map(|&group| [(group >> 8) as u8, group as u8, 0, 0])
        .collect();
    // The entropy image is a non-level-zero image and therefore starts
    // directly with its color-cache declaration.
    write_entropy_image_pixels(&mut writer, &entropy_pixels);

    // One fixed literal per group keeps the main data bit-free. The
    // groups are nevertheless written for every id through max_group,
    // including sparse group one below.
    for &pixel in group_pixels {
        for symbol in [pixel[1], pixel[0], pixel[2], pixel[3], 0] {
            write_simple_code(&mut writer, symbol);
        }
    }
    writer.into_bytes()
}

fn color_indexing_stream(
    width: u32,
    height: u32,
    palette_deltas: &[[u8; 4]],
    indexed_pixels: &[[u8; 4]],
) -> Vec<u8> {
    assert!((1..=256).contains(&palette_deltas.len()));
    let width_bits = crate::vp8l::header::color_index_width_bits(palette_deltas.len() as u16);
    let packed_width = width.div_ceil(1_u32 << width_bits);
    assert_eq!(
        indexed_pixels.len(),
        usize::try_from(packed_width * height).unwrap()
    );

    let mut writer = BitWriter::new();
    write_header(&mut writer, width, height, true);
    writer.write_bits(1, 1).unwrap(); // transform_present
    writer.write_bits(3, 2).unwrap(); // color indexing
    writer
        .write_bits(u32::try_from(palette_deltas.len() - 1).unwrap(), 8)
        .unwrap();
    write_entropy_image_pixels(&mut writer, palette_deltas);
    writer.write_bits(0, 1).unwrap(); // transform-list terminator
    write_entropy_image_pixels_at_level(&mut writer, indexed_pixels, true);
    writer.into_bytes()
}

fn all_transform_kinds_stream() -> Vec<u8> {
    let mut writer = BitWriter::new();
    write_header(&mut writer, 2, 1, true);

    writer.write_bits(1, 1).unwrap(); // predictor transform
    writer.write_bits(0, 2).unwrap();
    writer.write_bits(0, 3).unwrap(); // four-pixel blocks
    write_entropy_image_pixels(&mut writer, &[[0, 1, 0, 255]]);

    writer.write_bits(1, 1).unwrap(); // color transform
    writer.write_bits(1, 2).unwrap();
    writer.write_bits(0, 3).unwrap(); // four-pixel blocks
    write_entropy_image_pixels(&mut writer, &[[0, 0, 32, 0]]);

    writer.write_bits(1, 1).unwrap(); // subtract-green transform
    writer.write_bits(2, 2).unwrap();

    writer.write_bits(1, 1).unwrap(); // color indexing transform
    writer.write_bits(3, 2).unwrap();
    writer.write_bits(0, 8).unwrap(); // one palette entry
    write_entropy_image_pixels(&mut writer, &[[0, 32, 0, 0]]);

    writer.write_bits(0, 1).unwrap(); // transform-list terminator

    // Two one-bit palette indices, both zero, packed in green's low bits.
    write_entropy_image_pixels_at_level(&mut writer, &[[0, 0, 0, 0]], true);
    writer.into_bytes()
}

fn color_transform_stream(
    width: u32,
    height: u32,
    block_size_field: u8,
    transform_pixels: &[[u8; 4]],
    main_pixel: [u8; 4],
    following_transforms: &[u8],
) -> Vec<u8> {
    let block_size = 1_u32 << (u32::from(block_size_field) + 2);
    let table_width = width.div_ceil(block_size);
    let table_height = height.div_ceil(block_size);
    assert_eq!(
        transform_pixels.len(),
        usize::try_from(table_width * table_height).unwrap()
    );

    let mut writer = BitWriter::new();
    write_header(&mut writer, width, height, main_pixel[3] != 255);
    writer.write_bits(1, 1).unwrap(); // transform_present
    writer.write_bits(1, 2).unwrap(); // color transform
    writer.write_bits(u32::from(block_size_field), 3).unwrap();
    write_entropy_image_pixels(&mut writer, transform_pixels);
    for &transform in following_transforms {
        writer.write_bits(1, 1).unwrap(); // transform_present
        writer.write_bits(u32::from(transform), 2).unwrap();
    }
    writer.write_bits(0, 1).unwrap(); // transform-list terminator
    write_flat_entropy_image(&mut writer, main_pixel, true);
    writer.into_bytes()
}

fn predictor_stream(mode: u8) -> Vec<u8> {
    let mut writer = BitWriter::new();
    write_header(&mut writer, 2, 2, false);
    writer.write_bits(1, 1).unwrap(); // transform_present
    writer.write_bits(0, 2).unwrap(); // predictor transform
    writer.write_bits(0, 3).unwrap(); // 2 + 0 => four-pixel blocks

    // The predictor subimage is 1 by 1. It is a non-level-zero entropy
    // image, so this starts directly with color_cache_present; there is no
    // transform-list terminator or meta-Huffman flag here. Mode one is
    // carried in the green byte.
    write_flat_entropy_image(&mut writer, [0, mode, 0, 255], false);
    writer.write_bits(0, 1).unwrap(); // main transform-list terminator

    // All four residual samples are 1,1,1,0. Boundary rules reconstruct
    // the first row/column, while the lower-right pixel proves mode one
    // (left) is selected from the predictor subimage.
    write_flat_entropy_image(&mut writer, [1, 1, 1, 0], true);
    writer.into_bytes()
}

fn predictor_then_color_stream() -> Vec<u8> {
    let mut writer = BitWriter::new();
    write_header(&mut writer, 2, 2, false);
    writer.write_bits(1, 1).unwrap(); // predictor transform present
    writer.write_bits(0, 2).unwrap(); // predictor transform
    writer.write_bits(0, 3).unwrap(); // four-pixel blocks
    write_flat_entropy_image(&mut writer, [0, 1, 0, 255], false);
    writer.write_bits(1, 1).unwrap(); // color transform present
    writer.write_bits(1, 2).unwrap(); // color transform
    writer.write_bits(0, 3).unwrap(); // four-pixel blocks
    write_entropy_image_pixels(&mut writer, &[[0, 0, 32, 0]]);
    writer.write_bits(0, 1).unwrap(); // transform-list terminator
    write_flat_entropy_image(&mut writer, [0, 32, 0, 0], true);
    writer.into_bytes()
}

fn repeated_lz77_stream(width: u32, height: u32) -> Vec<u8> {
    let mut writer = BitWriter::new();
    write_header(&mut writer, width, height, false);
    writer.write_bits(0, 3).unwrap(); // no deferred features

    // Green symbol 2 is a literal. Green symbol 258 is length prefix 2,
    // which expands to a three-pixel copy.
    write_two_symbol_normal_code(&mut writer, GREEN_ALPHABET_SIZE, 2, 258);
    for symbol in [0, 0, 0] {
        write_simple_code(&mut writer, symbol);
    }
    write_simple_code(&mut writer, 0); // distance prefix 0 => code 1
    writer.write_bits(0, 1).unwrap(); // green literal symbol 2
    writer.write_bits(1, 1).unwrap(); // green copy symbol 258
    writer.into_bytes()
}

fn cache_hit_stream(pixel: [u8; 4], cache_bits: u8) -> Vec<u8> {
    let mut writer = BitWriter::new();
    write_header(&mut writer, 2, 1, pixel[3] != 255);
    writer.write_bits(0, 1).unwrap(); // transform_present
    writer.write_bits(1, 1).unwrap(); // color_cache_present
    writer.write_bits(u32::from(cache_bits), 4).unwrap(); // cache_bits
    writer.write_bits(0, 1).unwrap(); // use_meta_huffman

    let color = pack_argb(pixel[0], pixel[1], pixel[2], pixel[3]);
    let cache_index = crate::vp8l::color_cache::ColorCache::new(cache_bits)
        .unwrap()
        .index_of(color);
    let green = write_normal_code(
        &mut writer,
        GREEN_ALPHABET_SIZE + (1_usize << cache_bits),
        &[
            (usize::from(pixel[1]), 1),
            (GREEN_ALPHABET_SIZE + cache_index, 1),
        ],
    );
    for symbol in [pixel[0], pixel[2], pixel[3], 0] {
        write_simple_code(&mut writer, symbol);
    }
    write_symbol(&mut writer, &green, usize::from(pixel[1]));
    write_symbol(&mut writer, &green, GREEN_ALPHABET_SIZE + cache_index);
    writer.into_bytes()
}

fn cache_lz77_update_stream() -> (Vec<u8>, [u8; 2]) {
    let mut writer = BitWriter::new();
    write_header(&mut writer, 4, 1, true);
    writer.write_bits(0, 1).unwrap(); // transform_present
    writer.write_bits(1, 1).unwrap(); // color_cache_present
    writer.write_bits(1, 4).unwrap(); // cache_bits
    writer.write_bits(0, 1).unwrap(); // use_meta_huffman

    let cache = crate::vp8l::color_cache::ColorCache::new(1).unwrap();
    let first = 0_u8;
    let second = (1_u8..=u8::MAX)
        .find(|&alpha| {
            cache.index_of(pack_argb(0, 1, 0, alpha)) == cache.index_of(pack_argb(0, 1, 0, first))
        })
        .expect("a two-entry cache must have colliding alpha values");
    let cache_index = cache.index_of(pack_argb(0, 1, 0, first));

    let green = write_normal_code(
        &mut writer,
        GREEN_ALPHABET_SIZE + 2,
        &[(1, 1), (256, 2), (GREEN_ALPHABET_SIZE + cache_index, 2)],
    );
    write_simple_code(&mut writer, 0); // red
    write_simple_code(&mut writer, 0); // blue
    let alpha = write_normal_code(
        &mut writer,
        CHANNEL_ALPHABET_SIZE,
        &[(usize::from(first), 1), (usize::from(second), 1)],
    );
    write_simple_code(&mut writer, 13); // distance prefix => code 122 with extra 25

    write_symbol(&mut writer, &green, 1);
    write_symbol(&mut writer, &alpha, usize::from(first));
    write_symbol(&mut writer, &green, 1);
    write_symbol(&mut writer, &alpha, usize::from(second));
    write_symbol(&mut writer, &green, 256); // length prefix zero => one pixel
    writer.write_bits(25, 5).unwrap(); // distance prefix 13 => distance code 122 => distance two
    write_symbol(&mut writer, &green, GREEN_ALPHABET_SIZE + cache_index);
    (writer.into_bytes(), [first, second])
}

#[test]
fn decodes_a_handwritten_single_literal_pixel() {
    let data = literal_stream(1, 1, [0x12, 0x34, 0x56, 0x78]);
    let image = decode_literal_only(&data, &limits()).unwrap();
    assert_eq!((image.header.width, image.header.height), (1, 1));
    assert_eq!(image.rgba, [0x12, 0x34, 0x56, 0x78]);
}

#[test]
fn decodes_multiple_pixels_from_zero_bit_single_symbol_tables() {
    let data = literal_stream(3, 2, [1, 2, 3, 255]);
    let image = decode_literal_only(&data, &limits()).unwrap();
    assert_eq!(image.rgba, [1, 2, 3, 255].repeat(6));
}

#[test]
fn applies_subtract_green_to_handwritten_residual_pixels() {
    // Stored channels are residual red, green, residual blue, alpha.
    // Inversion adds green to red and blue modulo 256.
    let data = literal_stream_with_transforms(1, 1, [0xf0, 0x30, 0xee, 0x80], &[2]);
    let image = decode_literal_only(&data, &limits()).unwrap();
    assert_eq!(image.rgba, [0x20, 0x30, 0x1e, 0x80]);
}

#[test]
fn decodes_color_transform_with_specified_argb_multiplier_mapping() {
    // The transform pixel is ARGB on wire. R feeds red-to-blue, G feeds
    // green-to-blue, B feeds green-to-red, and alpha is ignored.
    let data = color_transform_stream(
        1,
        1,
        0,
        &[[0x20, 0x80, 0x01, 0x55]],
        [3, 0x80, 0, 0x44],
        &[],
    );
    let image = decode_no_transform(&data, &limits()).unwrap();
    // Green is signed -128. Red becomes 3 + (1 * -128 >> 5) = 255;
    // blue then receives (-128 * -128 >> 5) + (32 * -1 >> 5).
    assert_eq!(image.rgba, [255, 128, 255, 0x44]);
}

#[test]
fn color_transform_selects_multipliers_at_partial_block_boundaries() {
    // 5x5 pixels with four-pixel blocks yield a 2x2 transform image.
    // Each block carries a different green-to-red multiplier in B.
    let data = color_transform_stream(
        5,
        5,
        0,
        &[[0, 0, 0, 0], [0, 0, 1, 0], [0, 0, 2, 0], [0, 0, 0xff, 0]],
        [0, 32, 0, 1],
        &[],
    );
    let image = decode_no_transform(&data, &limits()).unwrap();
    let rgba_at = |x: usize, y: usize| &image.rgba[(y * 5 + x) * 4..(y * 5 + x + 1) * 4];
    assert_eq!(rgba_at(0, 0), [0, 32, 0, 1]);
    assert_eq!(rgba_at(4, 0), [1, 32, 0, 1]);
    assert_eq!(rgba_at(0, 4), [2, 32, 0, 1]);
    assert_eq!(rgba_at(4, 4), [255, 32, 0, 1]);
}

#[test]
fn inverse_transforms_run_in_reverse_wire_order_with_subtract_green() {
    // Color appears before subtract-green on wire, so subtract-green is
    // inverted first. Its reconstructed green then drives color's B=32
    // green-to-red multiplier.
    let data = color_transform_stream(1, 1, 0, &[[0, 0, 32, 0]], [0, 32, 0, 9], &[2]);
    let image = decode_no_transform(&data, &limits()).unwrap();
    assert_eq!(image.rgba, [64, 32, 32, 9]);
}

#[test]
fn inverse_transforms_run_in_reverse_wire_order_with_predictor() {
    // Predictor appears before color on wire, so color is inverted first.
    // The predictor then reconstructs each color-corrected residual.
    let image = decode_no_transform(&predictor_then_color_stream(), &limits()).unwrap();
    assert_eq!(
        image.rgba,
        [
            32, 32, 0, 255, // top-left boundary predictor
            64, 64, 0, 255, // top row uses left
            64, 64, 0, 255, // left column uses top
            96, 96, 0, 255, // mode one uses left
        ]
    );
}

#[test]
fn color_transform_storage_counts_toward_the_decoder_limit() {
    let data = color_transform_stream(
        5,
        5,
        0,
        &[[0, 0, 0, 0], [0, 0, 1, 0], [0, 0, 2, 0], [0, 0, 3, 0]],
        [0, 32, 0, 1],
        &[],
    );
    // Four compact multiplier entries (12 B), 25 packed main pixels
    // (100 B), and final RGBA (100 B) coexist during main entropy decode.
    let limited = DecodeLimits {
        max_alloc_bytes: 211,
        ..limits()
    };
    assert_eq!(
        decode_no_transform(&data, &limited).unwrap_err().kind(),
        DecodeErrorKind::LimitExceeded
    );
}

#[test]
fn decodes_predictor_subimage_without_a_nested_transform_list() {
    let image = decode_no_transform(&predictor_stream(1), &limits()).unwrap();
    assert_eq!(
        image.rgba,
        [
            1, 1, 1, 255, // top-left: opaque black + residual
            2, 2, 2, 255, // top row: left + residual
            2, 2, 2, 255, // left column: top + residual
            3, 3, 3, 255, // mode one: left + residual
        ]
    );
}

#[test]
fn rejects_predictor_modes_outside_the_wire_range() {
    assert_eq!(
        decode_no_transform(&predictor_stream(14), &limits())
            .unwrap_err()
            .kind(),
        DecodeErrorKind::InvalidBitstream
    );
}

#[test]
fn predictor_subimage_storage_counts_toward_the_allocation_limit() {
    // One packed predictor-mode pixel (4 B), four packed main pixels
    // (16 B), and final RGBA (16 B) are conservatively accounted while
    // main entropy is decoded.
    let limited = DecodeLimits {
        max_alloc_bytes: 35,
        ..limits()
    };
    assert_eq!(
        decode_no_transform(&predictor_stream(1), &limited)
            .unwrap_err()
            .kind(),
        DecodeErrorKind::LimitExceeded
    );
}

#[test]
fn decodes_meta_huffman_groups_with_round_up_and_sparse_ids() {
    // prefix_bits = 2 produces a 3x2 entropy image for this 9x5 output.
    // Group one appears only in the last column, while group two appears
    // in other blocks, so decoding must parse and retain all three groups
    // and must select them from the red/green 16-bit meta code.
    let group_map = [0_u16, 2, 1, 1, 0, 2];
    let group_pixels = [[1, 10, 100, 255], [2, 20, 110, 254], [3, 30, 120, 253]];
    let image = decode_no_transform(
        &meta_huffman_literal_stream(9, 5, 0, &group_map, &group_pixels),
        &limits(),
    )
    .unwrap();

    let mut expected = Vec::new();
    for y in 0..5_usize {
        for x in 0..9_usize {
            let group = group_map[(y / 4) * 3 + x / 4];
            expected.extend_from_slice(&group_pixels[usize::from(group)]);
        }
    }
    assert_eq!(image.rgba, expected);
}

#[test]
fn meta_huffman_group_id_uses_both_red_and_green_bytes() {
    // 0x0100 must select group 256, not group zero. The 256 preceding
    // groups are still present in the bitstream and must be parsed before
    // the selected group.
    let mut group_pixels = vec![[0, 0, 0, 0]; 257];
    group_pixels[256] = [9, 8, 7, 6];
    let image = decode_no_transform(
        &meta_huffman_literal_stream(1, 1, 0, &[0x0100], &group_pixels),
        &limits(),
    )
    .unwrap();
    assert_eq!(image.rgba, [9, 8, 7, 6]);
}

#[test]
fn meta_huffman_tables_and_maps_count_toward_allocation_limit() {
    let data = meta_huffman_literal_stream(1, 1, 0, &[0], &[[1, 2, 3, 4]]);
    let limited = DecodeLimits {
        // The nested entropy image itself is tiny; this limit is crossed
        // by the conservative retained/transient prefix-table accounting.
        max_alloc_bytes: 16 * 1024,
        ..limits()
    };
    assert_eq!(
        decode_no_transform(&data, &limited).unwrap_err().kind(),
        DecodeErrorKind::LimitExceeded
    );
}

#[test]
fn truncated_color_indexing_palette_reports_eof() {
    let mut writer = BitWriter::new();
    write_header(&mut writer, 1, 1, false);
    writer.write_bits(1, 1).unwrap(); // transform_present
    writer.write_bits(3, 2).unwrap(); // color indexing
    writer.write_bits(0, 8).unwrap(); // one palette entry
    assert_eq!(
        decode_literal_only(&writer.into_bytes(), &limits())
            .unwrap_err()
            .kind(),
        DecodeErrorKind::UnexpectedEof
    );
}

#[test]
fn decodes_packed_color_indices_and_palette_deltas() {
    // A four-entry palette packs four two-bit indices in each source
    // green byte.
    let data = color_indexing_stream(
        4,
        1,
        &[[10, 20, 30, 40], [5, 5, 5, 5], [7, 7, 7, 7], [9, 9, 9, 9]],
        &[[0xa5, 0b0100_0100, 0x5a, 0x33]],
    );
    let image = decode_no_transform(&data, &limits()).unwrap();
    let first = [10, 20, 30, 40];
    let second = [15, 25, 35, 45];
    assert_eq!(image.rgba, [first, second, first, second].concat());
}

#[test]
fn color_indexing_handles_each_palette_packing_boundary() {
    for (size, width) in [
        (2, 9_u32),
        (3, 5),
        (4, 5),
        (5, 3),
        (16, 3),
        (17, 3),
        (256, 3),
    ] {
        let palette = vec![[7, 8, 9, 10]; size];
        let width_bits = crate::vp8l::header::color_index_width_bits(size as u16);
        let packed_width = width.div_ceil(1_u32 << width_bits);
        let indexed = vec![[0, 0, 0, 0]; usize::try_from(packed_width).unwrap()];
        let image = decode_no_transform(
            &color_indexing_stream(width, 1, &palette, &indexed),
            &limits(),
        )
        .unwrap();
        assert_eq!(
            image.rgba,
            [7, 8, 9, 10].repeat(width as usize),
            "size {size}"
        );
    }
}

#[test]
fn invalid_packed_palette_indices_become_transparent_black() {
    // Palette size three selects two-bit indices. The first index is
    // three (invalid), while every remaining index is zero.
    let image = decode_no_transform(
        &color_indexing_stream(
            4,
            1,
            &[[1, 1, 1, 1], [1, 1, 1, 1], [1, 1, 1, 1]],
            &[[0xaa, 0b0000_0011, 0x55, 0x99]],
        ),
        &limits(),
    )
    .unwrap();
    assert_eq!(
        image.rgba,
        [
            0, 0, 0, 0, // invalid palette index
            1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
        ]
    );
}

#[test]
fn color_indexing_expands_before_other_inverse_transforms() {
    let image = decode_no_transform(&all_transform_kinds_stream(), &limits()).unwrap();
    // Wire order is predictor, color, subtract-green, indexing. Reverse
    // order expands palette indices first, then applies the other three
    // transforms at their original two-pixel width.
    assert_eq!(
        image.rgba,
        [
            64, 32, 32, 255, // opaque-black predictor boundary
            128, 64, 64, 255, // top-row left predictor
        ]
    );
}

#[test]
fn color_indexing_expansion_counts_packed_palette_and_output_buffers() {
    let data = color_indexing_stream(2, 1, &[[1, 2, 3, 4], [1, 2, 3, 4]], &[[0, 0, 0, 0]]);
    // The retained palette (8 B), narrow packed output (4 B), expanded
    // output (8 B), and final RGBA (8 B) coexist during expansion.
    let limited = DecodeLimits {
        max_alloc_bytes: 27,
        ..limits()
    };
    assert_eq!(
        decode_no_transform(&data, &limited).unwrap_err().kind(),
        DecodeErrorKind::LimitExceeded
    );
}

#[test]
fn truncated_predictor_subimage_reports_eof_after_prior_transforms() {
    let mut writer = BitWriter::new();
    write_header(&mut writer, 1, 1, false);
    writer.write_bits(1, 1).unwrap(); // subtract-green present
    writer.write_bits(2, 2).unwrap(); // subtract-green
    writer.write_bits(1, 1).unwrap(); // predictor present
    writer.write_bits(0, 2).unwrap(); // predictor
    writer.write_bits(0, 3).unwrap(); // predictor block_size_bits
    assert_eq!(
        decode_literal_only(&writer.into_bytes(), &limits())
            .unwrap_err()
            .kind(),
        DecodeErrorKind::UnexpectedEof
    );
}

#[test]
fn decodes_overlapping_lz77_copy_with_distance_one() {
    let data = repeated_lz77_stream(1, 4);
    let image = decode_literal_only(&data, &limits()).unwrap();
    assert_eq!(image.rgba, [0, 2, 0, 0].repeat(4));
}

#[test]
fn decodes_color_cache_hit_after_a_literal() {
    let pixel = [0x12, 0x34, 0x56, 0x78];
    let image = decode_no_transform(&cache_hit_stream(pixel, 1), &limits()).unwrap();
    assert_eq!(image.rgba, pixel.repeat(2));
}

#[test]
fn accepts_the_largest_color_cache_exponent() {
    let pixel = [0x12, 0x34, 0x56, 0x78];
    let image = decode_no_transform(&cache_hit_stream(pixel, 11), &limits()).unwrap();
    assert_eq!(image.rgba, pixel.repeat(2));
}

#[test]
fn cache_allocation_counts_toward_the_decoder_limit() {
    let pixel = [0x12, 0x34, 0x56, 0x78];
    let data = cache_hit_stream(pixel, 11);
    // Two packed pixels (8 B), 2048 cache entries (8192 B), and the two
    // RGBA pixels (8 B) coexist while final output is allocated.
    let limited = DecodeLimits {
        max_alloc_bytes: 8207,
        ..limits()
    };
    assert_eq!(
        decode_no_transform(&data, &limited).unwrap_err().kind(),
        DecodeErrorKind::LimitExceeded
    );
}

#[test]
fn lz77_pixels_update_color_cache_before_the_next_symbol() {
    let (data, alpha) = cache_lz77_update_stream();
    let image = decode_no_transform(&data, &limits()).unwrap();
    let first = [0, 1, 0, alpha[0]];
    let second = [0, 1, 0, alpha[1]];
    // The cache hit must resolve to `first`: the LZ77 reference copied it
    // after `second` had overwritten their shared cache slot.
    assert_eq!(image.rgba, [first, second, first, first].concat());
}

#[test]
fn rejects_invalid_or_truncated_color_cache_headers_without_panicking() {
    let mut invalid = cache_hit_stream([1, 2, 3, 4], 1);
    let cache_bits_position = HEADER_LEN * 8 + 2;
    for offset in 0..4 {
        let position = cache_bits_position + offset;
        invalid[position / 8] &= !(1 << (position % 8));
    }
    assert_eq!(
        decode_no_transform(&invalid, &limits()).unwrap_err().kind(),
        DecodeErrorKind::InvalidBitstream
    );

    let data = cache_hit_stream([1, 2, 3, 4], 1);
    for length in 0..data.len() {
        let result = decode_no_transform(&data[..length], &limits());
        assert!(
            result.is_err(),
            "truncation length {length} unexpectedly decoded"
        );
    }
}

#[test]
fn rejects_lz77_distance_before_produced_pixels() {
    // Distance code one means one scanline. At width two, it points back
    // two pixels although only the initial literal has been produced.
    let data = repeated_lz77_stream(2, 2);
    assert_eq!(
        decode_no_transform(&data, &limits()).unwrap_err().kind(),
        DecodeErrorKind::InvalidBitstream
    );
}

#[test]
fn rejects_lz77_copy_that_exceeds_image_output() {
    let data = repeated_lz77_stream(1, 3);
    assert_eq!(
        decode_no_transform(&data, &limits()).unwrap_err().kind(),
        DecodeErrorKind::InvalidBitstream
    );
}

#[test]
fn input_and_allocation_limits_apply_before_output_allocation() {
    let data = literal_stream(2, 2, [1, 2, 3, 4]);
    let input_limited = DecodeLimits {
        max_input_bytes: data.len() - 1,
        ..limits()
    };
    assert_eq!(
        decode_literal_only(&data, &input_limited)
            .unwrap_err()
            .kind(),
        DecodeErrorKind::LimitExceeded
    );
    let allocation_limited = DecodeLimits {
        max_alloc_bytes: 15,
        ..limits()
    };
    assert_eq!(
        decode_literal_only(&data, &allocation_limited)
            .unwrap_err()
            .kind(),
        DecodeErrorKind::LimitExceeded
    );
}

#[test]
fn work_budget_covers_headers_tables_and_literal_symbols() {
    let data = literal_stream(1, 1, [1, 2, 3, 4]);
    let limited = DecodeLimits {
        max_work_units: 12, // 3 stream flags + 5 tables + 4 channel symbols
        ..limits()
    };
    assert!(decode_literal_only(&data, &limited).is_ok());
    let exhausted = DecodeLimits {
        max_work_units: 11,
        ..limits()
    };
    assert_eq!(
        decode_literal_only(&data, &exhausted).unwrap_err().kind(),
        DecodeErrorKind::LimitExceeded
    );
}

#[test]
fn work_budget_covers_lz77_symbol_expansion_and_copy() {
    let data = repeated_lz77_stream(1, 4);
    let limited = DecodeLimits {
        // 3 flags + 5 tables + 4 literal symbols + 1 copy symbol +
        // length expansion + distance symbol + distance expansion + copy.
        max_work_units: 19,
        ..limits()
    };
    assert!(decode_no_transform(&data, &limited).is_ok());
    let exhausted = DecodeLimits {
        max_work_units: 18,
        ..limits()
    };
    assert_eq!(
        decode_no_transform(&data, &exhausted).unwrap_err().kind(),
        DecodeErrorKind::LimitExceeded
    );
}

#[test]
fn truncation_never_panics_and_reports_eof() {
    let data = literal_stream(1, 1, [1, 2, 3, 4]);
    for length in 0..data.len() {
        let error = decode_literal_only(&data[..length], &limits()).unwrap_err();
        assert_eq!(
            error.kind(),
            DecodeErrorKind::UnexpectedEof,
            "length {length}"
        );
    }

    let transformed = literal_stream_with_transforms(1, 1, [1, 2, 3, 4], &[2]);
    for length in 0..transformed.len() {
        let error = decode_literal_only(&transformed[..length], &limits()).unwrap_err();
        assert_eq!(
            error.kind(),
            DecodeErrorKind::UnexpectedEof,
            "subtract-green truncation length {length}"
        );
    }

    let color_transformed = color_transform_stream(1, 1, 0, &[[0, 0, 1, 0]], [1, 2, 3, 4], &[]);
    for length in 0..color_transformed.len() {
        let error = decode_no_transform(&color_transformed[..length], &limits()).unwrap_err();
        assert_eq!(
            error.kind(),
            DecodeErrorKind::UnexpectedEof,
            "color-transform truncation length {length}"
        );
    }
}
