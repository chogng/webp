use crate::vp8l::huffman::FastHuffmanTable;
use crate::vp8l::huffman::read_huffman_code;
use crate::vp8l::pixel::pack_argb;
use webp_core::BitReader;
use webp_core::DecodeError;
use webp_core::DecodeErrorKind;
use webp_core::WorkBudget;

pub(in crate::vp8l) const GREEN_ALPHABET_SIZE: usize = 256 + 24;
pub(in crate::vp8l) const CHANNEL_ALPHABET_SIZE: usize = 256;
pub(in crate::vp8l) const DISTANCE_ALPHABET_SIZE: usize = 40;

pub(in crate::vp8l) struct HuffmanCodes {
    pub(in crate::vp8l) green: FastHuffmanTable,
    pub(in crate::vp8l) red: FastHuffmanTable,
    pub(in crate::vp8l) blue: FastHuffmanTable,
    pub(in crate::vp8l) alpha: FastHuffmanTable,
    pub(in crate::vp8l) distance: FastHuffmanTable,
}

/// The maximum number of prefix tables in one VP8L meta-prefix group.
pub(in crate::vp8l) const HUFFMAN_TABLES_PER_GROUP: usize = 5;

// `HuffmanTable` is intentionally opaque to this crate. Reserve a deliberately
// conservative amount for every possible wire symbol so the allocation limit
// also covers the heap storage hidden behind its vectors. The root lookup
// table is a fixed heap allocation per table and is accounted separately.
pub(in crate::vp8l) const MAX_HUFFMAN_CODE_STORAGE_BYTES: usize = 64;

pub(in crate::vp8l) fn read_huffman_codes(
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

fn read_table(
    bits: &mut BitReader<'_>,
    budget: &mut WorkBudget,
    alphabet_size: usize,
) -> Result<FastHuffmanTable, DecodeError> {
    budget.consume(1)?;
    read_huffman_code(bits, alphabet_size)?.into_fast()
}

pub(in crate::vp8l) fn decode_fast_symbol(
    table: &FastHuffmanTable,
    bits: &mut webp_core::ShiftedBitReader<'_, '_>,
    budget: &mut WorkBudget,
) -> Result<usize, DecodeError> {
    budget.consume(1)?;
    if bits.available_bits() < 15 {
        bits.fill();
    }
    table.decode(bits).map(usize::from)
}

pub(in crate::vp8l) enum GreenOrLiteral {
    Green(usize),
    Literal(u32),
}

/// Decodes the four literal channels from one immutable bit-register snapshot
/// when every table has a packed representation. This removes three reader
/// state transitions and three repeated EOF checks from the dominant VP8L
/// path. Non-literals, rare fallback tables, and short tails retain the strict
/// per-symbol decoder.
#[inline]
pub(in crate::vp8l) fn decode_green_or_literal(
    codes: &HuffmanCodes,
    bits: &mut webp_core::ShiftedBitReader<'_, '_>,
    budget: &mut WorkBudget,
) -> Result<GreenOrLiteral, DecodeError> {
    budget.consume(1)?;
    if bits.available_bits() < 15 {
        bits.fill();
    }

    let lookahead = bits.peek_full();
    if let Some((green, green_bits)) = codes.green.lookup_buffered(lookahead as u16)
        && usize::from(green) < CHANNEL_ALPHABET_SIZE
    {
        let shifted = lookahead >> green_bits;
        if let Some((red, red_bits)) = codes.red.lookup_buffered(shifted as u16) {
            let used = green_bits + red_bits;
            let shifted = lookahead >> used;
            if let Some((blue, blue_bits)) = codes.blue.lookup_buffered(shifted as u16) {
                let used = used + blue_bits;
                let shifted = lookahead >> used;
                if let Some((alpha, alpha_bits)) = codes.alpha.lookup_buffered(shifted as u16) {
                    let used = used + alpha_bits;
                    if used <= bits.available_bits() {
                        budget.consume(3)?;
                        bits.consume_buffered(used)?;
                        return Ok(GreenOrLiteral::Literal(pack_argb(
                            red as u8,
                            green as u8,
                            blue as u8,
                            alpha as u8,
                        )));
                    }
                }
            }
        }
    }

    codes
        .green
        .decode(bits)
        .map(|symbol| GreenOrLiteral::Green(usize::from(symbol)))
}
