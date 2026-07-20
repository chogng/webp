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

/// Permutation used to transmit the code lengths of a normal VP8L Huffman
/// header.  The first `4 + ReadBits(4)` entries are present on the wire.
pub const CODE_LENGTH_CODE_ORDER: [usize; 19] = [
    17, 18, 0, 1, 2, 3, 4, 5, 16, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15,
];

const CODE_LENGTH_ALPHABET_SIZE: usize = CODE_LENGTH_CODE_ORDER.len();

/// Reads a simple VP8L Huffman code and builds its decoder table.
///
/// The enclosing Huffman-code parser must consume `simple_code_flag` before
/// calling this function.  The simple representation stores one or two
/// length-one symbols; the first is encoded in either one or eight bits and
/// the second is always encoded in eight bits.  Its symbols are constrained to
/// the caller-provided alphabet even though the wire representation itself is
/// limited to `[0, 255]`.
///
/// A two-symbol representation is permitted to repeat the same symbol by the
/// VP8L specification.  It remains a two-entry, one-bit decoder so the
/// subsequent wire stream remains aligned; both one-bit values decode to that
/// duplicated symbol.
pub fn read_simple_code(
    bits: &mut BitReader<'_>,
    alphabet_size: usize,
) -> Result<HuffmanTable, DecodeError> {
    if alphabet_size == 0 {
        return Err(invalid("VP8L Huffman alphabet must contain a symbol"));
    }

    let num_symbols = usize::from(bits.read_bit()?) + 1;
    debug_assert!((1..=2).contains(&num_symbols));

    let first_width = if bits.read_bit()? { 8 } else { 1 };
    let first_symbol = usize::try_from(bits.read_bits(first_width)?)
        .map_err(|_| invalid("VP8L simple Huffman symbol does not fit usize"))?;
    validate_simple_symbol(first_symbol, alphabet_size)?;

    let second_symbol = if num_symbols == 2 {
        let symbol = usize::try_from(bits.read_bits(8)?)
            .map_err(|_| invalid("VP8L simple Huffman symbol does not fit usize"))?;
        validate_simple_symbol(symbol, alphabet_size)?;
        Some(symbol)
    } else {
        None
    };

    Ok(HuffmanTable::from_simple_symbols(
        first_symbol,
        second_symbol,
    ))
}

fn validate_simple_symbol(symbol: usize, alphabet_size: usize) -> Result<(), DecodeError> {
    if symbol >= alphabet_size {
        return Err(invalid("VP8L simple Huffman symbol exceeds alphabet"));
    }
    Ok(())
}

/// Reads the code lengths for a normal (non-simple) VP8L Huffman code.
///
/// `alphabet_size` is the number of symbols in the code that is being
/// described.  The returned vector has exactly that many entries.  Repeat
/// symbols are checked before extending the output, so an attacker-controlled
/// repeat can never grow it beyond this bound.
///
/// This routine parses only the normal representation.  The leading
/// `simple_code_flag` belongs to the enclosing Huffman-code parser and must be
/// consumed by that caller before invoking this function.
pub fn read_normal_code_lengths(
    bits: &mut BitReader<'_>,
    alphabet_size: usize,
) -> Result<Vec<u8>, DecodeError> {
    if alphabet_size == 0 {
        return Err(invalid("VP8L Huffman alphabet must contain a symbol"));
    }

    let num_code_lengths = usize::try_from(bits.read_bits(4)?)
        .map_err(|_| invalid("VP8L code-length count does not fit usize"))?
        + 4;
    debug_assert!((4..=CODE_LENGTH_ALPHABET_SIZE).contains(&num_code_lengths));

    let mut code_length_lengths = [0_u8; CODE_LENGTH_ALPHABET_SIZE];
    for &symbol in CODE_LENGTH_CODE_ORDER.iter().take(num_code_lengths) {
        code_length_lengths[symbol] = bits.read_bits(3)? as u8;
    }

    // Besides decoding the following stream, construction validates that this
    // compact header cannot represent an over-subscribed or incomplete tree.
    let code_length_table = HuffmanTable::from_code_lengths(&code_length_lengths)?;

    let mut lengths = Vec::new();
    lengths.try_reserve_exact(alphabet_size).map_err(|_| {
        DecodeError::new(
            DecodeErrorKind::AllocationFailed,
            None,
            "VP8L code-length output allocation failed",
        )
    })?;
    let mut previous_nonzero = None;

    while lengths.len() < alphabet_size {
        let symbol = code_length_table.decode(bits)?;
        match symbol {
            0..=15 => {
                lengths.push(symbol as u8);
                if symbol != 0 {
                    previous_nonzero = Some(symbol as u8);
                }
            }
            16 => {
                let repeat = usize::try_from(bits.read_bits(2)?)
                    .map_err(|_| invalid("VP8L repeat-16 count does not fit usize"))?
                    + 3;
                // VP8L assigns the value eight when repeat-16 occurs before
                // any nonzero length.  Keep the last nonzero value separately
                // so leading zero runs do not change that default.
                let value = previous_nonzero.unwrap_or(8);
                extend_repeat(&mut lengths, alphabet_size, value, repeat)?;
            }
            17 => {
                let repeat = usize::try_from(bits.read_bits(3)?)
                    .map_err(|_| invalid("VP8L repeat-17 count does not fit usize"))?
                    + 3;
                extend_repeat(&mut lengths, alphabet_size, 0, repeat)?;
            }
            18 => {
                let repeat = usize::try_from(bits.read_bits(7)?)
                    .map_err(|_| invalid("VP8L repeat-18 count does not fit usize"))?
                    + 11;
                extend_repeat(&mut lengths, alphabet_size, 0, repeat)?;
            }
            _ => return Err(invalid("VP8L code-length symbol is out of range")),
        }
    }

    Ok(lengths)
}

fn extend_repeat(
    lengths: &mut Vec<u8>,
    alphabet_size: usize,
    value: u8,
    repeat: usize,
) -> Result<(), DecodeError> {
    let end = lengths
        .len()
        .checked_add(repeat)
        .ok_or_else(|| invalid("VP8L code-length repeat count overflow"))?;
    if end > alphabet_size {
        return Err(invalid("VP8L code-length repeat exceeds alphabet"));
    }
    lengths.resize(end, value);
    Ok(())
}

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
    /// Builds the decoder represented by a VP8L simple code header.
    ///
    /// With one symbol the format consumes no data bits.  With two symbols it
    /// uses the two length-one canonical codes (zero then one), even if the
    /// symbols are the same.
    #[must_use]
    pub fn from_simple_symbols(first_symbol: usize, second_symbol: Option<usize>) -> Self {
        match second_symbol {
            None => Self {
                codes: vec![Code {
                    bits: 0,
                    length: 0,
                    symbol: first_symbol,
                }],
                max_code_length: 0,
            },
            Some(second_symbol) => Self {
                codes: vec![
                    Code {
                        bits: 0,
                        length: 1,
                        symbol: first_symbol,
                    },
                    Code {
                        bits: 1,
                        length: 1,
                        symbol: second_symbol,
                    },
                ],
                max_code_length: 1,
            },
        }
    }

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

    fn write_normal_header(writer: &mut BitWriter, includes_one: bool) {
        // Use all entries through index eight in CODE_LENGTH_CODE_ORDER so the
        // code-length alphabet contains 0, 16, 17 and 18.  The optional 1
        // makes it possible to exercise repeat-16 after a nonzero length.
        writer.write_bits(5, 4).unwrap(); // 4 + 5 == 9 entries.
        let on_wire_lengths = if includes_one {
            // symbols 17, 18, 0 have length 2; 1 and 16 have length 3.
            [2, 2, 2, 3, 0, 0, 0, 0, 3]
        } else {
            // symbols 17, 18, 0 and 16 form a balanced length-two tree.
            [2, 2, 2, 0, 0, 0, 0, 0, 2]
        };
        for length in on_wire_lengths {
            writer.write_bits(length, 3).unwrap();
        }
    }

    fn write_code_length_symbol(writer: &mut BitWriter, includes_one: bool, symbol: usize) {
        let mut lengths = [0_u8; CODE_LENGTH_ALPHABET_SIZE];
        lengths[0] = 2;
        lengths[16] = if includes_one { 3 } else { 2 };
        lengths[17] = 2;
        lengths[18] = 2;
        if includes_one {
            lengths[1] = 3;
        }
        let (code, width) = wire_code(&lengths, symbol);
        writer.write_bits(code, width).unwrap();
    }

    fn normal_stream(includes_one: bool, entries: &[(usize, u32, u8)]) -> Vec<u8> {
        let mut writer = BitWriter::new();
        write_normal_header(&mut writer, includes_one);
        for &(symbol, extra, width) in entries {
            write_code_length_symbol(&mut writer, includes_one, symbol);
            writer.write_bits(extra, width).unwrap();
        }
        writer.into_bytes()
    }

    fn normal_stream_with_all_literal_lengths() -> Vec<u8> {
        // A complete 19-symbol code-length alphabet: 13 leaves at depth four
        // and six at depth five.  Writing the lengths in wire order makes this
        // test fail if even one kCodeLengthCodeOrder position is changed.
        let mut table_lengths = [5_u8; CODE_LENGTH_ALPHABET_SIZE];
        for length in table_lengths.iter_mut().take(13) {
            *length = 4;
        }
        let mut writer = BitWriter::new();
        writer.write_bits(15, 4).unwrap(); // 4 + 15 == all 19 entries.
        for &symbol in &CODE_LENGTH_CODE_ORDER {
            writer
                .write_bits(u32::from(table_lengths[symbol]), 3)
                .unwrap();
        }
        for symbol in 0..=15 {
            let (code, width) = wire_code(&table_lengths, symbol);
            writer.write_bits(code, width).unwrap();
        }
        writer.into_bytes()
    }

    fn simple_stream(
        has_second_symbol: bool,
        first_uses_eight_bits: bool,
        first_symbol: u32,
        second_symbol: u32,
    ) -> BitWriter {
        let mut writer = BitWriter::new();
        writer.write_bits(u32::from(has_second_symbol), 1).unwrap();
        writer
            .write_bits(u32::from(first_uses_eight_bits), 1)
            .unwrap();
        writer
            .write_bits(first_symbol, if first_uses_eight_bits { 8 } else { 1 })
            .unwrap();
        if has_second_symbol {
            writer.write_bits(second_symbol, 8).unwrap();
        }
        writer
    }

    /// Produces an exact bit-prefix of `stream`, with the reader positioned
    /// after leading padding so its remaining input is exactly `available`
    /// bits.  This lets truncation tests cover every bit boundary, rather than
    /// only byte-aligned file cuts.
    fn simple_prefix(stream: &BitWriter, available: usize) -> (Vec<u8>, usize) {
        assert!(available <= stream.bit_len());
        let leading_padding = (8 - (available % 8)) % 8;
        let mut writer = BitWriter::new();
        writer.write_bits(0, leading_padding as u8).unwrap();
        for bit_index in 0..available {
            let bit = u32::from((stream.as_bytes()[bit_index / 8] >> (bit_index % 8)) & 1);
            writer.write_bits(bit, 1).unwrap();
        }
        assert_eq!(writer.bit_len() % 8, 0);
        (writer.into_bytes(), leading_padding)
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

    #[test]
    fn simple_code_decodes_one_or_two_symbols_in_lsb_order() {
        let one = simple_stream(false, false, 1, 0);
        let mut one_input = BitReader::new(one.as_bytes());
        let one_table = read_simple_code(&mut one_input, 2).unwrap();
        assert_eq!(one_table.symbol_count(), 1);
        assert_eq!(one_table.max_code_length(), 0);
        assert_eq!(one_table.decode(&mut one_input), Ok(1));
        assert_eq!(one_input.bit_position(), one.bit_len());

        // For two length-one entries canonical code zero belongs to the first
        // symbol and canonical code one to the second, unchanged in LSB order.
        let two = simple_stream(true, true, 3, 200);
        let mut two_input = BitReader::new(two.as_bytes());
        let two_table = read_simple_code(&mut two_input, 256).unwrap();
        let data_start = two_input.bit_position();
        let mut data = BitReader::new(&[0b0000_0010]);
        assert_eq!(two_table.decode(&mut data), Ok(3));
        assert_eq!(two_table.decode(&mut data), Ok(200));
        assert_eq!(two_input.bit_position(), data_start);
    }

    #[test]
    fn simple_code_honors_symbol_widths_and_boundaries() {
        for &(first_uses_eight_bits, first_symbol) in
            &[(false, 0), (false, 1), (true, 0), (true, 255)]
        {
            let stream = simple_stream(false, first_uses_eight_bits, first_symbol, 0);
            let table = read_simple_code(&mut BitReader::new(stream.as_bytes()), 256).unwrap();
            let mut no_data = BitReader::new(&[]);
            assert_eq!(table.decode(&mut no_data), Ok(first_symbol as usize));
        }

        for &(first_symbol, second_symbol) in &[(0, 255), (255, 0)] {
            let stream = simple_stream(true, true, first_symbol, second_symbol);
            let table = read_simple_code(&mut BitReader::new(stream.as_bytes()), 256).unwrap();
            let mut input = BitReader::new(&[0b0000_0010]);
            assert_eq!(table.decode(&mut input), Ok(first_symbol as usize));
            assert_eq!(table.decode(&mut input), Ok(second_symbol as usize));
        }
    }

    #[test]
    fn simple_code_allows_duplicate_symbols() {
        let stream = simple_stream(true, true, 42, 42);
        let mut input = BitReader::new(stream.as_bytes());
        let table = read_simple_code(&mut input, 256).unwrap();
        assert_eq!(table.symbol_count(), 2);
        assert_eq!(table.max_code_length(), 1);
        assert_eq!(input.bit_position(), stream.bit_len());
        let mut data = BitReader::new(&[0b0000_0010]);
        assert_eq!(table.decode(&mut data), Ok(42));
        assert_eq!(data.bit_position(), 1);
        assert_eq!(table.decode(&mut data), Ok(42));
        assert_eq!(data.bit_position(), 2);
    }

    #[test]
    fn simple_code_rejects_symbols_outside_the_target_alphabet() {
        let first = simple_stream(false, true, 2, 0);
        let error = read_simple_code(&mut BitReader::new(first.as_bytes()), 2).unwrap_err();
        assert_eq!(error.kind(), DecodeErrorKind::InvalidBitstream);
        assert_eq!(
            error.context(),
            "VP8L simple Huffman symbol exceeds alphabet"
        );

        let second = simple_stream(true, false, 1, 2);
        let error = read_simple_code(&mut BitReader::new(second.as_bytes()), 2).unwrap_err();
        assert_eq!(error.kind(), DecodeErrorKind::InvalidBitstream);
        assert_eq!(
            error.context(),
            "VP8L simple Huffman symbol exceeds alphabet"
        );

        let error = read_simple_code(&mut BitReader::new(&[]), 0).unwrap_err();
        assert_eq!(error.kind(), DecodeErrorKind::InvalidBitstream);
    }

    #[test]
    fn simple_code_reports_truncation_at_every_field_boundary() {
        let streams = [
            simple_stream(false, false, 1, 0),
            simple_stream(false, true, 255, 0),
            simple_stream(true, false, 1, 255),
            simple_stream(true, true, 255, 0),
        ];
        for stream in &streams {
            for available in 0..stream.bit_len() {
                let (prefix, start) = simple_prefix(stream, available);
                let mut input = BitReader::with_bit_position(&prefix, start).unwrap();
                let error = read_simple_code(&mut input, 256).unwrap_err();
                assert_eq!(
                    error.kind(),
                    DecodeErrorKind::UnexpectedEof,
                    "bits={}/{}",
                    available,
                    stream.bit_len()
                );
            }
        }
    }

    #[test]
    fn normal_code_lengths_use_the_wire_order_and_decode_repeats() {
        // 17 with extra=7 means ten zeros, exactly filling the alphabet.
        let stream = normal_stream(false, &[(17, 7, 3)]);
        assert_eq!(
            read_normal_code_lengths(&mut BitReader::new(&stream), 10).unwrap(),
            vec![0; 10]
        );
    }

    #[test]
    fn normal_code_lengths_decode_every_literal_length_and_all_order_slots() {
        let stream = normal_stream_with_all_literal_lengths();
        assert_eq!(
            read_normal_code_lengths(&mut BitReader::new(&stream), 16).unwrap(),
            (0_u8..=15).collect::<Vec<_>>()
        );
    }

    #[test]
    fn repeat_sixteen_defaults_to_eight_before_any_nonzero_length() {
        // Leading zeros deliberately do not replace the required default.
        let stream = normal_stream(false, &[(17, 0, 3), (16, 0, 2)]);
        assert_eq!(
            read_normal_code_lengths(&mut BitReader::new(&stream), 6).unwrap(),
            vec![0, 0, 0, 8, 8, 8]
        );
    }

    #[test]
    fn repeat_sixteen_uses_the_previous_nonzero_length() {
        let stream = normal_stream(true, &[(1, 0, 0), (16, 0, 2)]);
        assert_eq!(
            read_normal_code_lengths(&mut BitReader::new(&stream), 4).unwrap(),
            vec![1, 1, 1, 1]
        );
    }

    #[test]
    fn repeat_seventeen_and_eighteen_cover_their_format_bounds() {
        let minimum_zero_run = normal_stream(false, &[(17, 0, 3)]);
        assert_eq!(
            read_normal_code_lengths(&mut BitReader::new(&minimum_zero_run), 3).unwrap(),
            vec![0; 3]
        );

        let maximum_zero_run = normal_stream(false, &[(18, 127, 7)]);
        assert_eq!(
            read_normal_code_lengths(&mut BitReader::new(&maximum_zero_run), 138).unwrap(),
            vec![0; 138]
        );
    }

    #[test]
    fn repeat_bounds_are_checked_before_the_output_is_extended() {
        let stream = normal_stream(false, &[(16, 3, 2)]); // repeat 6
        assert_eq!(
            read_normal_code_lengths(&mut BitReader::new(&stream), 6).unwrap(),
            vec![8; 6]
        );
        for alphabet_size in [5, 2] {
            let error =
                read_normal_code_lengths(&mut BitReader::new(&stream), alphabet_size).unwrap_err();
            assert_eq!(error.kind(), DecodeErrorKind::InvalidBitstream);
            assert_eq!(error.context(), "VP8L code-length repeat exceeds alphabet");
        }
    }

    #[test]
    fn normal_header_truncation_is_never_treated_as_a_valid_tree() {
        let stream = normal_stream(false, &[(16, 0, 2)]);
        for cut in 0..stream.len() {
            let error =
                read_normal_code_lengths(&mut BitReader::new(&stream[..cut]), 3).unwrap_err();
            assert_eq!(error.kind(), DecodeErrorKind::UnexpectedEof, "cut={cut}");
        }
    }

    #[test]
    fn normal_header_rejects_over_and_under_subscribed_code_length_trees() {
        let mut over = BitWriter::new();
        over.write_bits(0, 4).unwrap();
        for length in [1, 1, 1, 0] {
            over.write_bits(length, 3).unwrap();
        }
        let error = read_normal_code_lengths(&mut BitReader::new(over.as_bytes()), 1).unwrap_err();
        assert_eq!(error.context(), "VP8L Huffman tree is oversubscribed");

        let mut under = BitWriter::new();
        under.write_bits(0, 4).unwrap();
        for length in [2, 2, 0, 0] {
            under.write_bits(length, 3).unwrap();
        }
        let error = read_normal_code_lengths(&mut BitReader::new(under.as_bytes()), 1).unwrap_err();
        assert_eq!(error.context(), "VP8L Huffman tree is incomplete");
    }

    #[test]
    fn normal_code_lengths_reject_an_empty_target_alphabet() {
        let error = read_normal_code_lengths(&mut BitReader::new(&[]), 0).unwrap_err();
        assert_eq!(error.kind(), DecodeErrorKind::InvalidBitstream);
    }
}
