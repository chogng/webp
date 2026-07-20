#![forbid(unsafe_code)]
//! Canonical Huffman decoding primitives for VP8L.
//!
//! The VP8L entropy stream is read least-significant bit first.  Canonical
//! codes are conventionally assigned most-significant bit first.  VP8L emits
//! their bits least-significant bit first, so the table stores each canonical
//! code with exactly its significant bits reversed.

use webp_core::{BitReader, DecodeError, DecodeErrorKind};

/// The longest code allowed by the VP8L format.
pub const MAX_CODE_LENGTH: u8 = 15;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Code {
    bits: u16,
    length: u8,
    symbol: usize,
}

/// A validated canonical Huffman table with symbols addressed by input index.
///
/// Construct a table with [`Self::from_code_lengths`].  A table must form a
/// complete prefix tree, with the VP8L exception of exactly one length-one
/// symbol.  This makes malformed or ambiguous entropy headers fail before any
/// symbol decoding begins.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HuffmanTable {
    codes: Vec<Code>,
    max_code_length: u8,
}

impl HuffmanTable {
    /// Builds a canonical table from the code length for each symbol.
    ///
    /// A zero length marks an unused symbol.  The returned symbol value is the
    /// corresponding index in `code_lengths`.
    pub fn from_code_lengths(code_lengths: &[u8]) -> Result<Self, DecodeError> {
        let mut counts = [0_usize; MAX_CODE_LENGTH as usize + 1];
        let mut symbols = 0_usize;
        let mut only_length = 0_u8;

        for &length in code_lengths {
            if length > MAX_CODE_LENGTH {
                return Err(invalid("VP8L Huffman code length exceeds 15 bits"));
            }
            if length != 0 {
                counts[usize::from(length)] = counts[usize::from(length)]
                    .checked_add(1)
                    .ok_or_else(|| invalid("VP8L Huffman symbol count overflow"))?;
                symbols = symbols
                    .checked_add(1)
                    .ok_or_else(|| invalid("VP8L Huffman symbol count overflow"))?;
                only_length = length;
            }
        }

        if symbols == 0 {
            return Err(invalid("VP8L Huffman tree has no symbols"));
        }

        // VP8L encodes a one-symbol alphabet without consuming a bit.  Its
        // parsed code length is still required to be one so malformed,
        // incomplete trees cannot silently become this special representation.
        if symbols == 1 {
            if only_length != 1 {
                return Err(invalid(
                    "VP8L one-symbol Huffman tree must have a length-one code",
                ));
            }
            let symbol = code_lengths
                .iter()
                .position(|&length| length != 0)
                .ok_or_else(|| invalid("VP8L Huffman tree has no symbols"))?;
            return Ok(Self {
                codes: vec![Code {
                    bits: 0,
                    length: 0,
                    symbol,
                }],
                max_code_length: 0,
            });
        }

        let mut unused_leaves = 1_i32;
        for (length, &count) in counts.iter().enumerate().skip(1) {
            unused_leaves = unused_leaves
                .checked_mul(2)
                .and_then(|value| value.checked_sub(i32::try_from(count).ok()?))
                .ok_or_else(|| invalid("VP8L Huffman tree is oversubscribed"))?;
            if unused_leaves < 0 {
                return Err(invalid("VP8L Huffman tree is oversubscribed"));
            }
            debug_assert!(length <= usize::from(MAX_CODE_LENGTH));
        }

        if unused_leaves != 0 {
            return Err(invalid("VP8L Huffman tree is incomplete"));
        }

        let mut next_code = [0_u16; MAX_CODE_LENGTH as usize + 1];
        let mut code = 0_u16;
        for length in 1..=usize::from(MAX_CODE_LENGTH) {
            code = code
                .checked_add(u16::try_from(counts[length - 1]).map_err(|_| {
                    invalid("VP8L Huffman code count does not fit canonical representation")
                })?)
                .ok_or_else(|| invalid("VP8L Huffman canonical code overflow"))?
                << 1;
            next_code[length] = code;
        }

        let mut codes = Vec::new();
        codes.try_reserve_exact(symbols).map_err(|_| {
            DecodeError::new(
                DecodeErrorKind::AllocationFailed,
                None,
                "VP8L Huffman table allocation failed",
            )
        })?;

        let mut max_code_length = 0_u8;
        for (symbol, &length) in code_lengths.iter().enumerate() {
            if length == 0 {
                continue;
            }
            let slot = &mut next_code[usize::from(length)];
            let bits = reverse_code(*slot, length);
            *slot = slot
                .checked_add(1)
                .ok_or_else(|| invalid("VP8L Huffman canonical code overflow"))?;
            codes.push(Code {
                bits,
                length,
                symbol,
            });
            max_code_length = max_code_length.max(length);
        }

        Ok(Self {
            codes,
            max_code_length,
        })
    }

    /// Decodes a single symbol from a least-significant-bit-first VP8L stream.
    pub fn decode(&self, bits: &mut BitReader<'_>) -> Result<usize, DecodeError> {
        if self.max_code_length == 0 {
            // Construction guarantees this is the VP8L single-symbol case.
            return Ok(self.codes[0].symbol);
        }
        let mut code = 0_u16;
        for length in 1..=self.max_code_length {
            let bit = u16::from(bits.read_bit()?);
            code |= bit << (length - 1);
            if let Some(entry) = self
                .codes
                .iter()
                .find(|entry| entry.length == length && entry.bits == code)
            {
                return Ok(entry.symbol);
            }
        }
        Err(invalid("VP8L Huffman code does not exist in table"))
    }

    /// Number of symbols with a nonzero code length.
    #[must_use]
    pub fn symbol_count(&self) -> usize {
        self.codes.len()
    }

    /// Longest code in this table.
    #[must_use]
    pub const fn max_code_length(&self) -> u8 {
        self.max_code_length
    }
}

fn invalid(context: &'static str) -> DecodeError {
    DecodeError::new(DecodeErrorKind::InvalidBitstream, None, context)
}

fn reverse_code(code: u16, length: u8) -> u16 {
    code.reverse_bits() >> (u16::BITS - u32::from(length))
}

#[cfg(test)]
mod tests {
    use super::*;
    use webp_core::BitWriter;

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
                return (u32::from(reverse_code(code as u16, length)), length);
            }
            code += 1;
            previous_length = length;
        }
        panic!("requested unused symbol");
    }

    fn encoded_symbols(lengths: &[u8], symbols: &[usize]) -> Vec<u8> {
        let mut writer = BitWriter::new();
        for &symbol in symbols {
            let (code, length) = wire_code(lengths, symbol);
            writer.write_bits(code, length).unwrap();
        }
        writer.into_bytes()
    }

    #[test]
    fn decodes_single_symbol_without_consuming_a_bit() {
        let table = HuffmanTable::from_code_lengths(&[0, 1, 0]).unwrap();
        let mut input = BitReader::new(&[]);
        assert_eq!(table.symbol_count(), 1);
        assert_eq!(table.max_code_length(), 0);
        assert_eq!(table.decode(&mut input), Ok(1));
        assert_eq!(input.bit_position(), 0);
    }

    #[test]
    fn decodes_balanced_table_in_lsb_order() {
        let lengths = [2, 2, 2, 2];
        let table = HuffmanTable::from_code_lengths(&lengths).unwrap();
        let encoded = encoded_symbols(&lengths, &[0, 1, 2, 3, 3, 0]);
        let mut input = BitReader::new(&encoded);
        for expected in [0, 1, 2, 3, 3, 0] {
            assert_eq!(table.decode(&mut input), Ok(expected));
        }
    }

    #[test]
    fn balanced_table_uses_bit_reversed_canonical_wire_codes() {
        // Canonical codes are 00, 01, 10, 11.  VP8L writes them as 00, 10,
        // 01, 11, yielding this byte in least-significant-bit-first order.
        let table = HuffmanTable::from_code_lengths(&[2, 2, 2, 2]).unwrap();
        let mut input = BitReader::new(&[0b1101_1000]);
        for expected in 0..4 {
            assert_eq!(table.decode(&mut input), Ok(expected));
        }
    }

    #[test]
    fn decodes_unbalanced_complete_table() {
        let lengths = [1, 2, 3, 3];
        let table = HuffmanTable::from_code_lengths(&lengths).unwrap();
        let encoded = encoded_symbols(&lengths, &[3, 0, 2, 1, 0, 3]);
        let mut input = BitReader::new(&encoded);
        for expected in [3, 0, 2, 1, 0, 3] {
            assert_eq!(table.decode(&mut input), Ok(expected));
        }
    }

    #[test]
    fn rejects_oversubscribed_tree() {
        let error = HuffmanTable::from_code_lengths(&[1, 1, 1]).unwrap_err();
        assert_eq!(error.kind(), DecodeErrorKind::InvalidBitstream);
        assert_eq!(error.context(), "VP8L Huffman tree is oversubscribed");
    }

    #[test]
    fn rejects_incomplete_trees() {
        for lengths in [&[][..], &[2, 2][..], &[2][..]] {
            let error = HuffmanTable::from_code_lengths(lengths).unwrap_err();
            assert_eq!(error.kind(), DecodeErrorKind::InvalidBitstream);
        }
    }

    #[test]
    fn rejects_code_lengths_past_vp8l_limit() {
        let error = HuffmanTable::from_code_lengths(&[MAX_CODE_LENGTH + 1]).unwrap_err();
        assert_eq!(error.kind(), DecodeErrorKind::InvalidBitstream);
    }

    #[test]
    fn truncation_is_reported_without_panic() {
        let table = HuffmanTable::from_code_lengths(&[1, 2, 3, 3]).unwrap();
        // Symbol 3 needs three one-bits, but only two remain after this cursor.
        let mut input = BitReader::with_bit_position(&[0b1100_0000], 6).unwrap();
        let error = table.decode(&mut input).unwrap_err();
        assert_eq!(error.kind(), DecodeErrorKind::UnexpectedEof);
    }
}
