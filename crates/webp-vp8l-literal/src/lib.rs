#![forbid(unsafe_code)]
//! A deliberately small, bounded VP8L entropy decoder.
//!
//! This crate is an integration seam for the first lossless decoder slice. It
//! accepts only a single Huffman group, no transforms, no color cache and
//! literal green symbols.  All other valid VP8L features receive an explicit
//! [`DecodeErrorKind::UnsupportedFeature`] rather than being partially
//! interpreted.  The output uses straight RGBA byte order.

use webp_core::{
    BitReader, DecodeError, DecodeErrorKind, DecodeLimits, WorkBudget, checked_image_bytes,
};
use webp_vp8l::{HEADER_LEN, Vp8lHeader, parse_header};
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

/// Decodes a standalone VP8L stream limited to its literal-only subset.
///
/// The input begins with the five-byte VP8L fixed header.  `transform`, color
/// cache, meta-Huffman groups, and backward references are intentionally not
/// implemented in this stage.  The entropy syntax still contains all five
/// Huffman codes required by VP8L: green, red, blue, alpha, and distance.
pub fn decode_literal_only(
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
    if bits.read_bit()? {
        return Err(unsupported("VP8L color cache is not implemented"));
    }

    budget.consume(1)?;
    if bits.read_bit()? {
        return Err(unsupported("VP8L meta-Huffman groups are not implemented"));
    }

    let codes = read_huffman_codes(&mut bits, &mut budget)?;
    let pixels =
        usize::try_from(u64::from(header.width) * u64::from(header.height)).map_err(|_| {
            DecodeError::new(
                DecodeErrorKind::LimitExceeded,
                None,
                "image pixel count does not fit platform usize",
            )
        })?;
    let mut rgba = Vec::new();
    rgba.try_reserve_exact(rgba_len).map_err(|_| {
        DecodeError::new(
            DecodeErrorKind::AllocationFailed,
            None,
            "RGBA output allocation failed",
        )
    })?;

    for _ in 0..pixels {
        let green = decode_symbol(&codes.green, &mut bits, &mut budget)?;
        if green >= CHANNEL_ALPHABET_SIZE {
            return Err(unsupported("VP8L backward references are not implemented"));
        }
        let red = decode_symbol(&codes.red, &mut bits, &mut budget)?;
        let blue = decode_symbol(&codes.blue, &mut bits, &mut budget)?;
        let alpha = decode_symbol(&codes.alpha, &mut bits, &mut budget)?;
        debug_assert!(red < CHANNEL_ALPHABET_SIZE);
        debug_assert!(blue < CHANNEL_ALPHABET_SIZE);
        debug_assert!(alpha < CHANNEL_ALPHABET_SIZE);
        rgba.extend_from_slice(&[red as u8, green as u8, blue as u8, alpha as u8]);
    }

    Ok(LiteralImage { header, rgba })
}

struct HuffmanCodes {
    green: HuffmanTable,
    red: HuffmanTable,
    blue: HuffmanTable,
    alpha: HuffmanTable,
    // This is required even in literal-only data. Keeping it parsed ensures a
    // caller cannot accidentally accept a truncated fifth table.
    _distance: HuffmanTable,
}

fn read_huffman_codes(
    bits: &mut BitReader<'_>,
    budget: &mut WorkBudget,
) -> Result<HuffmanCodes, DecodeError> {
    Ok(HuffmanCodes {
        green: read_table(bits, budget, GREEN_ALPHABET_SIZE)?,
        red: read_table(bits, budget, CHANNEL_ALPHABET_SIZE)?,
        blue: read_table(bits, budget, CHANNEL_ALPHABET_SIZE)?,
        alpha: read_table(bits, budget, CHANNEL_ALPHABET_SIZE)?,
        _distance: read_table(bits, budget, DISTANCE_ALPHABET_SIZE)?,
    })
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
        for bit in 0..3 {
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
    fn rejects_a_literal_stream_with_a_backward_reference_symbol() {
        let mut writer = BitWriter::new();
        write_header(&mut writer, 1, 1, false);
        writer.write_bits(0, 3).unwrap();
        writer.write_bits(0, 1).unwrap(); // green normal_code_flag
        writer.write_bits(0, 4).unwrap(); // four code-length alphabet entries
        // Wire order is 17, 18, 0, 1.  Symbols 0 and 1 form a complete tree.
        for length in [0_u32, 0, 1, 1] {
            writer.write_bits(length, 3).unwrap();
        }
        // Emit 256 zero lengths, a length-one code for green symbol 256, then
        // the remaining 23 zero lengths. In the tiny code-length table, wire
        // code zero means symbol zero and wire code one means symbol one.
        for _ in 0..256 {
            writer.write_bits(0, 1).unwrap();
        }
        writer.write_bits(1, 1).unwrap();
        for _ in 0..23 {
            writer.write_bits(0, 1).unwrap();
        }
        for symbol in [0, 0, 0, 0] {
            write_simple_code(&mut writer, symbol);
        }
        assert_eq!(
            decode_literal_only(&writer.into_bytes(), &limits())
                .unwrap_err()
                .kind(),
            DecodeErrorKind::UnsupportedFeature
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
