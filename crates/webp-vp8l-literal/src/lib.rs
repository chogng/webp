#![forbid(unsafe_code)]
//! A deliberately small, bounded VP8L entropy decoder.
//!
//! This crate is an integration seam for the first lossless decoder slice. It
//! accepts only a single Huffman group, no transforms, optional color cache,
//! and literal/backward-reference entropy symbols.  All other valid VP8L features receive an explicit
//! [`DecodeErrorKind::UnsupportedFeature`] rather than being partially
//! interpreted.  The output uses straight RGBA byte order.

use webp_core::{
    BitReader, DecodeError, DecodeErrorKind, DecodeLimits, WorkBudget, checked_image_bytes,
};
use webp_vp8l::{HEADER_LEN, Vp8lHeader, parse_header};
use webp_vp8l_color_cache::{ColorCacheOutput, MAX_COLOR_CACHE_BITS, MIN_COLOR_CACHE_BITS};
use webp_vp8l_entropy::{
    copy_lz77_pixels, decode_distance, decode_length, distance_code_to_distance,
};
use webp_vp8l_huffman::{HuffmanTable, read_huffman_code};

const GREEN_ALPHABET_SIZE: usize = 256 + 24;
const CHANNEL_ALPHABET_SIZE: usize = 256;
const DISTANCE_ALPHABET_SIZE: usize = 40;

/// A decoded straight/unpremultiplied RGBA image.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiteralImage {
    /// Fixed VP8L image information.
    pub header: Vp8lHeader,
    /// Pixels in row-major RGBA8 byte order.
    pub rgba: Vec<u8>,
}

/// Decodes a standalone VP8L stream limited to one entropy group.
///
/// The input begins with the five-byte VP8L fixed header.  `transform`, color
/// Transforms and meta-Huffman groups are intentionally not implemented in
/// this stage. For compatibility with the initial decoder slice this function
/// delegates to [`decode_no_transform`], which additionally supports VP8L
/// backward references and color-cache references.
pub fn decode_literal_only(
    data: &[u8],
    limits: &DecodeLimits,
) -> Result<LiteralImage, DecodeError> {
    decode_no_transform(data, limits)
}

/// Decodes a standalone VP8L stream with one Huffman group and no transforms.
///
/// Literal pixels, green-alphabet backward-reference symbols, and color-cache
/// references are supported. Transforms and meta-Huffman groups remain
/// explicitly unsupported. Internally decoded samples are packed as
/// `0xAARRGGBB` until entropy expansion is complete, then emitted in RGBA byte
/// order.
pub fn decode_no_transform(
    data: &[u8],
    limits: &DecodeLimits,
) -> Result<LiteralImage, DecodeError> {
    let header = parse_header(data, limits)?;
    let rgba_len = checked_image_bytes(header.width, header.height, 4)?;
    if rgba_len > limits.max_alloc_bytes {
        return Err(DecodeError::new(
            DecodeErrorKind::LimitExceeded,
            None,
            "RGBA output exceeds configured allocation limit",
        ));
    }

    let mut bits = BitReader::with_bit_position(data, HEADER_LEN * 8)?;
    let mut budget = limits.work_budget();

    budget.consume(1)?;
    if bits.read_bit()? {
        return Err(unsupported("VP8L transforms are not implemented"));
    }

    budget.consume(1)?;
    let color_cache_bits = if bits.read_bit()? {
        budget.consume(1)?;
        Some(bits.read_bits(4)? as u8)
    } else {
        None
    };

    budget.consume(1)?;
    if bits.read_bit()? {
        return Err(unsupported("VP8L meta-Huffman groups are not implemented"));
    }

    let color_cache_size = match color_cache_bits {
        None => 0,
        Some(cache_bits) => {
            if !(MIN_COLOR_CACHE_BITS..=MAX_COLOR_CACHE_BITS).contains(&cache_bits) {
                return Err(DecodeError::new(
                    DecodeErrorKind::InvalidBitstream,
                    None,
                    "VP8L color-cache bits must be in 1..=11",
                ));
            }
            1_usize << cache_bits
        }
    };
    let pixels =
        usize::try_from(u64::from(header.width) * u64::from(header.height)).map_err(|_| {
            DecodeError::new(
                DecodeErrorKind::LimitExceeded,
                None,
                "image pixel count does not fit platform usize",
            )
        })?;
    check_allocation_budget(pixels, rgba_len, color_cache_size, limits.max_alloc_bytes)?;

    let codes = read_huffman_codes(&mut bits, &mut budget, color_cache_size)?;
    let mut output = PixelOutput::new(color_cache_bits, pixels)?;

    while output.len() < pixels {
        let green = decode_symbol(&codes.green, &mut bits, &mut budget)?;
        if green < CHANNEL_ALPHABET_SIZE {
            let red = decode_symbol(&codes.red, &mut bits, &mut budget)?;
            let blue = decode_symbol(&codes.blue, &mut bits, &mut budget)?;
            let alpha = decode_symbol(&codes.alpha, &mut bits, &mut budget)?;
            debug_assert!(red < CHANNEL_ALPHABET_SIZE);
            debug_assert!(blue < CHANNEL_ALPHABET_SIZE);
            debug_assert!(alpha < CHANNEL_ALPHABET_SIZE);
            output.emit_literal(pack_argb(red as u8, green as u8, blue as u8, alpha as u8))?;
            continue;
        }

        if green >= GREEN_ALPHABET_SIZE {
            let cache_index = green - GREEN_ALPHABET_SIZE;
            if cache_index >= color_cache_size {
                return Err(DecodeError::new(
                    DecodeErrorKind::InvalidBitstream,
                    None,
                    "VP8L color-cache symbol exceeds cache size",
                ));
            }
            output.emit_cache_hit(cache_index)?;
            continue;
        }

        let length_prefix = u8::try_from(green - CHANNEL_ALPHABET_SIZE).map_err(|_| {
            DecodeError::new(
                DecodeErrorKind::InvalidBitstream,
                None,
                "VP8L length prefix does not fit u8",
            )
        })?;
        let length = decode_length(&mut bits, &mut budget, length_prefix)?;
        let distance_prefix = decode_symbol(&codes.distance, &mut bits, &mut budget)?;
        let distance_prefix = u8::try_from(distance_prefix).map_err(|_| {
            DecodeError::new(
                DecodeErrorKind::InvalidBitstream,
                None,
                "VP8L distance prefix does not fit u8",
            )
        })?;
        let distance_code = decode_distance(&mut bits, &mut budget, distance_prefix)?;
        let distance = distance_code_to_distance(distance_code, header.width)?;
        output.copy_lz77(length, distance, pixels, &mut budget)?;
    }

    let mut rgba = Vec::new();
    rgba.try_reserve_exact(rgba_len).map_err(|_| {
        DecodeError::new(
            DecodeErrorKind::AllocationFailed,
            None,
            "RGBA output allocation failed",
        )
    })?;
    for pixel in output.into_pixels() {
        rgba.extend_from_slice(&unpack_rgba(pixel));
    }

    Ok(LiteralImage { header, rgba })
}

/// Bounds the allocations that coexist while entropy output becomes RGBA.
///
/// The packed ARGB output, optional color-cache entries, and final RGBA bytes
/// all coexist while the decoder allocates the final image. This deliberately
/// treats vector capacities as their maximum configured sizes, avoiding an
/// allocation-limit bypass through a tiny image paired with a large cache.
fn check_allocation_budget(
    pixels: usize,
    rgba_len: usize,
    color_cache_size: usize,
    max_alloc_bytes: usize,
) -> Result<(), DecodeError> {
    let packed_bytes = pixels.checked_mul(size_of::<u32>()).ok_or_else(|| {
        DecodeError::new(
            DecodeErrorKind::LimitExceeded,
            None,
            "packed VP8L output byte size overflow",
        )
    })?;
    let cache_bytes = color_cache_size
        .checked_mul(size_of::<u32>())
        .ok_or_else(|| {
            DecodeError::new(
                DecodeErrorKind::LimitExceeded,
                None,
                "VP8L color-cache byte size overflow",
            )
        })?;
    let total = packed_bytes
        .checked_add(cache_bytes)
        .and_then(|value| value.checked_add(rgba_len))
        .ok_or_else(|| {
            DecodeError::new(
                DecodeErrorKind::LimitExceeded,
                None,
                "VP8L decoder allocation budget overflow",
            )
        })?;
    if total > max_alloc_bytes {
        return Err(DecodeError::new(
            DecodeErrorKind::LimitExceeded,
            None,
            "VP8L decoder allocations exceed configured allocation limit",
        ));
    }
    Ok(())
}

const fn pack_argb(red: u8, green: u8, blue: u8, alpha: u8) -> u32 {
    ((alpha as u32) << 24) | ((red as u32) << 16) | ((green as u32) << 8) | (blue as u32)
}

const fn unpack_rgba(pixel: u32) -> [u8; 4] {
    [
        (pixel >> 16) as u8,
        (pixel >> 8) as u8,
        pixel as u8,
        (pixel >> 24) as u8,
    ]
}

struct HuffmanCodes {
    green: HuffmanTable,
    red: HuffmanTable,
    blue: HuffmanTable,
    alpha: HuffmanTable,
    distance: HuffmanTable,
}

fn read_huffman_codes(
    bits: &mut BitReader<'_>,
    budget: &mut WorkBudget,
    color_cache_size: usize,
) -> Result<HuffmanCodes, DecodeError> {
    Ok(HuffmanCodes {
        green: read_table(
            bits,
            budget,
            GREEN_ALPHABET_SIZE
                .checked_add(color_cache_size)
                .ok_or_else(|| {
                    DecodeError::new(
                        DecodeErrorKind::InvalidBitstream,
                        None,
                        "VP8L color-cache alphabet size overflow",
                    )
                })?,
        )?,
        red: read_table(bits, budget, CHANNEL_ALPHABET_SIZE)?,
        blue: read_table(bits, budget, CHANNEL_ALPHABET_SIZE)?,
        alpha: read_table(bits, budget, CHANNEL_ALPHABET_SIZE)?,
        distance: read_table(bits, budget, DISTANCE_ALPHABET_SIZE)?,
    })
}

enum PixelOutput {
    Plain(Vec<u32>),
    Cached(ColorCacheOutput),
}

impl PixelOutput {
    fn new(color_cache_bits: Option<u8>, pixels: usize) -> Result<Self, DecodeError> {
        match color_cache_bits {
            Some(bits) => Ok(Self::Cached(
                ColorCacheOutput::with_cache_bits_and_capacity(bits, pixels)?,
            )),
            None => {
                let mut output = Vec::new();
                output.try_reserve_exact(pixels).map_err(|_| {
                    DecodeError::new(
                        DecodeErrorKind::AllocationFailed,
                        None,
                        "packed VP8L output allocation failed",
                    )
                })?;
                Ok(Self::Plain(output))
            }
        }
    }

    fn len(&self) -> usize {
        match self {
            Self::Plain(pixels) => pixels.len(),
            Self::Cached(output) => output.pixels().len(),
        }
    }

    fn emit_literal(&mut self, color: u32) -> Result<(), DecodeError> {
        match self {
            Self::Plain(pixels) => {
                pixels.try_reserve(1).map_err(|_| {
                    DecodeError::new(
                        DecodeErrorKind::AllocationFailed,
                        None,
                        "packed VP8L output allocation failed",
                    )
                })?;
                pixels.push(color);
                Ok(())
            }
            Self::Cached(output) => output.emit_literal(color),
        }
    }

    fn emit_cache_hit(&mut self, index: usize) -> Result<(), DecodeError> {
        match self {
            Self::Plain(_) => Err(DecodeError::new(
                DecodeErrorKind::InvalidBitstream,
                None,
                "VP8L color-cache symbol appeared without a color cache",
            )),
            Self::Cached(output) => output.emit_cache_hit(index).map(|_| ()),
        }
    }

    fn copy_lz77(
        &mut self,
        length: usize,
        distance: usize,
        output_limit: usize,
        budget: &mut WorkBudget,
    ) -> Result<(), DecodeError> {
        match self {
            Self::Plain(pixels) => copy_lz77_pixels(pixels, length, distance, output_limit, budget),
            Self::Cached(output) => output.copy_lz77(length, distance, output_limit, budget),
        }
    }

    fn into_pixels(self) -> Vec<u32> {
        match self {
            Self::Plain(pixels) => pixels,
            Self::Cached(output) => output.into_parts().0,
        }
    }
}

fn read_table(
    bits: &mut BitReader<'_>,
    budget: &mut WorkBudget,
    alphabet_size: usize,
) -> Result<HuffmanTable, DecodeError> {
    budget.consume(1)?;
    read_huffman_code(bits, alphabet_size)
}

fn decode_symbol(
    table: &HuffmanTable,
    bits: &mut BitReader<'_>,
    budget: &mut WorkBudget,
) -> Result<usize, DecodeError> {
    budget.consume(1)?;
    table.decode(bits)
}

fn unsupported(context: &'static str) -> DecodeError {
    DecodeError::new(DecodeErrorKind::UnsupportedFeature, None, context)
}

#[cfg(test)]
mod tests {
    use super::*;
    use webp_core::BitWriter;
    use webp_vp8l::SIGNATURE;

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
        let mut writer = BitWriter::new();
        write_header(&mut writer, width, height, pixel[3] != 255);
        writer.write_bits(0, 1).unwrap(); // transform_present
        writer.write_bits(0, 1).unwrap(); // color_cache_present
        writer.write_bits(0, 1).unwrap(); // use_meta_huffman
        for symbol in [pixel[1], pixel[0], pixel[2], pixel[3], 0] {
            write_simple_code(&mut writer, symbol);
        }
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
        let cache_index = webp_vp8l_color_cache::ColorCache::new(cache_bits)
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

        let cache = webp_vp8l_color_cache::ColorCache::new(1).unwrap();
        let first = 0_u8;
        let second = (1_u8..=u8::MAX)
            .find(|&alpha| {
                cache.index_of(pack_argb(0, 1, 0, alpha))
                    == cache.index_of(pack_argb(0, 1, 0, first))
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
    fn rejects_each_deferred_feature_before_entropy_decode() {
        // The middle deferred feature is the supported color-cache flag. The
        // transform and meta-Huffman flags still fail before entropy decode.
        for bit in [0, 2] {
            let mut data = literal_stream(1, 1, [1, 2, 3, 4]);
            let position = HEADER_LEN * 8 + bit;
            data[position / 8] |= 1 << (position % 8);
            assert_eq!(
                decode_literal_only(&data, &limits()).unwrap_err().kind(),
                DecodeErrorKind::UnsupportedFeature
            );
        }
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
    }
}
