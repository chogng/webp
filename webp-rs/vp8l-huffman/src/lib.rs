#![forbid(unsafe_code)]
//! Canonical Huffman decoding primitives for VP8L.
//!
//! The VP8L entropy stream is read least-significant bit first.  Canonical
//! codes are conventionally assigned most-significant bit first.  VP8L emits
//! their bits least-significant bit first, so the table stores each canonical
//! code with exactly its significant bits reversed.

use webp_core::BitReader;
use webp_core::DecodeError;
use webp_core::DecodeErrorKind;
use webp_core::ShiftedBitReader;

/// The longest code allowed by the VP8L format.
pub const MAX_CODE_LENGTH: u8 = 15;

const ROOT_TABLE_BITS: u8 = 8;
const ROOT_TABLE_SIZE: usize = 1 << ROOT_TABLE_BITS;
const FAST_ROOT_TABLE_BITS: u8 = 10;
const FAST_ENTRY_VALUE_MASK: u16 = 0x0fff;

/// Permutation used to transmit the code lengths of a normal VP8L Huffman
/// header.  The first `4 + ReadBits(4)` entries are present on the wire.
pub const CODE_LENGTH_CODE_ORDER: [usize; 19] = [
    17, 18, 0, 1, 2, 3, 4, 5, 16, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15,
];

const CODE_LENGTH_ALPHABET_SIZE: usize = CODE_LENGTH_CODE_ORDER.len();

/// Reads a complete VP8L Huffman code header and builds its decoder table.
///
/// Each code begins with `simple_code_flag`.  A set flag selects the compact
/// simple representation; a clear flag selects the normal code-length
/// representation.  The flag is read before dispatching so a truncated header
/// is always reported as [`DecodeErrorKind::UnexpectedEof`], rather than being
/// mistaken for either representation.
pub fn read_huffman_code(
    bits: &mut BitReader<'_>,
    alphabet_size: usize,
) -> Result<HuffmanTable, DecodeError> {
    if bits.read_bit()? {
        read_simple_code(bits, alphabet_size)
    } else {
        let lengths = read_normal_code_lengths(bits, alphabet_size)?;
        HuffmanTable::from_code_lengths(&lengths)
    }
}

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
/// described.  The returned vector has exactly that many entries.  A normal
/// header may use VP8L's `max_symbol` form to limit the number of encoded
/// code-length symbols. A repeat is one encoded symbol even though it expands
/// to several output entries. The omitted suffix is zero-filled, and expanded
/// repeats are always bounded by the complete target alphabet.
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

    // VP8L can shorten a sparse code-length stream by explicitly limiting the
    // number of code-length symbols read from the Huffman-coded stream. A
    // repeat code consumes one of these symbols but may expand to several
    // entries in the final alphabet. Entries left after the limit is reached
    // are implicit zeros.
    let max_code_length_symbols = if bits.read_bit()? {
        let length_nbits = 2 + 2 * bits.read_bits(3)? as u8;
        let max_symbol = usize::try_from(bits.read_bits(length_nbits)?)
            .map_err(|_| invalid("VP8L code-length max symbol does not fit usize"))?
            .checked_add(2)
            .ok_or_else(|| invalid("VP8L code-length max symbol overflow"))?;
        if max_symbol > alphabet_size {
            return Err(invalid("VP8L code-length max symbol exceeds alphabet"));
        }
        max_symbol
    } else {
        alphabet_size
    };

    let mut lengths = Vec::new();
    lengths.try_reserve_exact(alphabet_size).map_err(|_| {
        DecodeError::new(
            DecodeErrorKind::AllocationFailed,
            None,
            "VP8L code-length output allocation failed",
        )
    })?;
    let mut previous_nonzero = None;
    let mut code_length_symbols_remaining = max_code_length_symbols;

    while lengths.len() < alphabet_size && code_length_symbols_remaining != 0 {
        code_length_symbols_remaining -= 1;
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

    lengths.resize(alphabet_size, 0);

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
struct RootTableEntry {
    symbol_index: u16,
    bits: u8,
    secondary_bits: u8,
}

const EMPTY_ROOT_ENTRY: RootTableEntry = RootTableEntry {
    symbol_index: 0,
    bits: 0,
    secondary_bits: 0,
};

/// Heap storage used by one Huffman table's replicated root lookup table.
///
/// Consumers that impose a decoded-data allocation limit should include this
/// alongside the table's inline representation and symbol vector.
pub const ROOT_TABLE_STORAGE_BYTES: usize =
    core::mem::size_of::<[RootTableEntry; ROOT_TABLE_SIZE]>();

/// Maximum heap storage for all compact second-level entries of one table.
pub const MAX_SECONDARY_TABLE_STORAGE_BYTES: usize = core::mem::size_of::<RootTableEntry>()
    * ROOT_TABLE_SIZE
    * (1 << (MAX_CODE_LENGTH - ROOT_TABLE_BITS));

fn empty_root_table() -> Box<[RootTableEntry; ROOT_TABLE_SIZE]> {
    Box::new([EMPTY_ROOT_ENTRY; ROOT_TABLE_SIZE])
}

/// A validated canonical Huffman table with symbols addressed by input index.
///
/// Construct a table with [`Self::from_code_lengths`].  A table must form a
/// complete prefix tree, with the VP8L exception of exactly one length-one
/// symbol.  This makes malformed or ambiguous entropy headers fail before any
/// symbol decoding begins.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HuffmanTable {
    // Symbols are ordered by code length and then by symbol value. Canonical
    // codes with a given length occupy a contiguous numeric range, allowing a
    // decoded code to select its symbol without scanning the entire alphabet.
    symbols: Vec<usize>,
    first_code: [u32; MAX_CODE_LENGTH as usize + 1],
    first_symbol: [usize; MAX_CODE_LENGTH as usize + 1],
    code_count: [usize; MAX_CODE_LENGTH as usize + 1],
    // Most VP8L codes fit in this table. Entries are replicated over all
    // unused high bits, so a direct hit consumes exactly the code length.
    root_table: Box<[RootTableEntry; ROOT_TABLE_SIZE]>,
    secondary_table: Vec<RootTableEntry>,
    max_code_length: u8,
}

/// A packed VP8L symbol table for dense pixel entropy decoding.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FastHuffmanTable(FastHuffmanTableInner);

#[derive(Clone, Debug, Eq, PartialEq)]
enum FastHuffmanTableInner {
    Single(u16),
    Packed {
        root_bits: u8,
        root_mask: u16,
        root: Vec<u16>,
        secondary: Vec<u16>,
    },
    Fallback(Box<HuffmanTable>),
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
                symbols: vec![first_symbol],
                first_code: [0; MAX_CODE_LENGTH as usize + 1],
                first_symbol: [0; MAX_CODE_LENGTH as usize + 1],
                code_count: [0; MAX_CODE_LENGTH as usize + 1],
                root_table: empty_root_table(),
                secondary_table: Vec::new(),
                max_code_length: 0,
            },
            Some(second_symbol) => {
                let mut table = Self {
                    symbols: vec![first_symbol, second_symbol],
                    first_code: [0; MAX_CODE_LENGTH as usize + 1],
                    first_symbol: [0; MAX_CODE_LENGTH as usize + 1],
                    code_count: [0; MAX_CODE_LENGTH as usize + 1],
                    root_table: empty_root_table(),
                    secondary_table: Vec::new(),
                    max_code_length: 1,
                };
                table.code_count[1] = 2;
                fill_root_table(&mut table.root_table, 0, 1, 0);
                fill_root_table(&mut table.root_table, 1, 1, 1);
                table
            }
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
                symbols: vec![symbol],
                first_code: [0; MAX_CODE_LENGTH as usize + 1],
                first_symbol: [0; MAX_CODE_LENGTH as usize + 1],
                code_count: [0; MAX_CODE_LENGTH as usize + 1],
                root_table: empty_root_table(),
                secondary_table: Vec::new(),
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

        let mut first_code = [0_u32; MAX_CODE_LENGTH as usize + 1];
        let mut code = 0_u32;
        for length in 1..=usize::from(MAX_CODE_LENGTH) {
            code = code
                .checked_add(u32::try_from(counts[length - 1]).map_err(|_| {
                    invalid("VP8L Huffman code count does not fit canonical representation")
                })?)
                .and_then(|value| value.checked_shl(1))
                .ok_or_else(|| invalid("VP8L Huffman canonical code overflow"))?;
            first_code[length] = code;
        }

        let mut symbols_by_code = Vec::new();
        symbols_by_code.try_reserve_exact(symbols).map_err(|_| {
            DecodeError::new(
                DecodeErrorKind::AllocationFailed,
                None,
                "VP8L Huffman table allocation failed",
            )
        })?;

        let mut first_symbol = [0_usize; MAX_CODE_LENGTH as usize + 1];
        let mut next_symbol = 0_usize;
        for length in 1..=usize::from(MAX_CODE_LENGTH) {
            first_symbol[length] = next_symbol;
            next_symbol = next_symbol
                .checked_add(counts[length])
                .ok_or_else(|| invalid("VP8L Huffman symbol index overflow"))?;
        }

        let mut root_table = empty_root_table();
        let mut secondary_bits = [0_u8; ROOT_TABLE_SIZE];
        let mut sizing_codes = first_code;
        for (length, next_code_for_length) in sizing_codes
            .iter_mut()
            .enumerate()
            .take(usize::from(MAX_CODE_LENGTH) + 1)
            .skip(usize::from(ROOT_TABLE_BITS + 1))
        {
            for &symbol_length in code_lengths {
                if usize::from(symbol_length) != length {
                    continue;
                }
                let wire_code = reverse_code(
                    u16::try_from(*next_code_for_length)
                        .map_err(|_| invalid("VP8L Huffman canonical code exceeds 15 bits"))?,
                    symbol_length,
                );
                let prefix = usize::from(wire_code) & (ROOT_TABLE_SIZE - 1);
                secondary_bits[prefix] =
                    secondary_bits[prefix].max(symbol_length - ROOT_TABLE_BITS);
                *next_code_for_length = next_code_for_length
                    .checked_add(1)
                    .ok_or_else(|| invalid("VP8L Huffman canonical code overflow"))?;
            }
        }

        let mut secondary_offsets = [0_usize; ROOT_TABLE_SIZE];
        let mut secondary_len = 0_usize;
        for (prefix, &table_bits) in secondary_bits.iter().enumerate() {
            if table_bits == 0 {
                continue;
            }
            secondary_offsets[prefix] = secondary_len;
            root_table[prefix] = RootTableEntry {
                symbol_index: u16::try_from(secondary_len)
                    .map_err(|_| invalid("VP8L Huffman secondary offset does not fit u16"))?,
                bits: 0,
                secondary_bits: table_bits,
            };
            secondary_len = secondary_len
                .checked_add(1_usize << table_bits)
                .ok_or_else(|| invalid("VP8L Huffman secondary table size overflow"))?;
        }
        let mut secondary_table = Vec::new();
        secondary_table
            .try_reserve_exact(secondary_len)
            .map_err(|_| {
                DecodeError::new(
                    DecodeErrorKind::AllocationFailed,
                    None,
                    "VP8L Huffman secondary table allocation failed",
                )
            })?;
        secondary_table.resize(secondary_len, EMPTY_ROOT_ENTRY);

        let mut next_code = first_code;
        let mut max_code_length = 0_u8;
        for (length, next_code_for_length) in next_code
            .iter_mut()
            .enumerate()
            .take(usize::from(MAX_CODE_LENGTH) + 1)
            .skip(1)
        {
            for (symbol, &symbol_length) in code_lengths.iter().enumerate() {
                if usize::from(symbol_length) != length {
                    continue;
                }
                let canonical_code = *next_code_for_length;
                let wire_code = reverse_code(
                    u16::try_from(canonical_code)
                        .map_err(|_| invalid("VP8L Huffman canonical code exceeds 15 bits"))?,
                    symbol_length,
                );
                if symbol_length <= ROOT_TABLE_BITS {
                    let symbol_index = u16::try_from(symbols_by_code.len())
                        .map_err(|_| invalid("VP8L Huffman root symbol index does not fit u16"))?;
                    fill_root_table(&mut root_table, wire_code, symbol_length, symbol_index);
                } else {
                    let prefix = usize::from(wire_code) & (ROOT_TABLE_SIZE - 1);
                    let symbol_index = u16::try_from(symbols_by_code.len()).map_err(|_| {
                        invalid("VP8L Huffman secondary symbol index does not fit u16")
                    })?;
                    fill_secondary_table(
                        &mut secondary_table,
                        secondary_offsets[prefix],
                        secondary_bits[prefix],
                        wire_code,
                        symbol_length,
                        symbol_index,
                    );
                }
                *next_code_for_length = next_code_for_length
                    .checked_add(1)
                    .ok_or_else(|| invalid("VP8L Huffman canonical code overflow"))?;
                symbols_by_code.push(symbol);
                max_code_length = max_code_length.max(symbol_length);
            }
        }

        Ok(Self {
            symbols: symbols_by_code,
            first_code,
            first_symbol,
            code_count: counts,
            root_table,
            secondary_table,
            max_code_length,
        })
    }

    /// Decodes a single symbol from a least-significant-bit-first VP8L stream.
    #[inline]
    pub fn decode(&self, bits: &mut BitReader<'_>) -> Result<usize, DecodeError> {
        if self.max_code_length == 0 {
            // Construction guarantees this is the VP8L single-symbol case.
            return Ok(self.symbols[0]);
        }

        if bits.remaining_bits() >= usize::from(ROOT_TABLE_BITS) {
            let prefix = usize::try_from(bits.read_bits(ROOT_TABLE_BITS)?)
                .map_err(|_| invalid("VP8L Huffman root table index does not fit usize"))?;
            let entry = self.root_table[prefix];
            if entry.bits != 0 {
                if entry.bits < ROOT_TABLE_BITS {
                    bits.rewind_bits(ROOT_TABLE_BITS - entry.bits)?;
                }
                return self
                    .symbols
                    .get(usize::from(entry.symbol_index))
                    .copied()
                    .ok_or_else(|| invalid("VP8L Huffman root symbol index is missing"));
            }

            if entry.secondary_bits != 0
                && bits.remaining_bits() >= usize::from(entry.secondary_bits)
            {
                let index = usize::try_from(bits.read_bits(entry.secondary_bits)?)
                    .map_err(|_| invalid("VP8L Huffman secondary index does not fit usize"))?;
                let secondary = self
                    .secondary_table
                    .get(usize::from(entry.symbol_index) + index)
                    .copied()
                    .ok_or_else(|| invalid("VP8L Huffman secondary table index is missing"))?;
                if secondary.bits == 0 {
                    return Err(invalid("VP8L Huffman secondary table entry is empty"));
                }
                if secondary.bits < entry.secondary_bits {
                    bits.rewind_bits(entry.secondary_bits - secondary.bits)?;
                }
                return self
                    .symbols
                    .get(usize::from(secondary.symbol_index))
                    .copied()
                    .ok_or_else(|| invalid("VP8L Huffman secondary symbol index is missing"));
            }

            let mut code = u16::try_from(prefix)
                .map_err(|_| invalid("VP8L Huffman prefix does not fit u16"))?;
            for length in ROOT_TABLE_BITS + 1..=self.max_code_length {
                code |= u16::from(bits.read_bit()?) << (length - 1);
                if let Some(symbol) = self.symbol_for_code(code, length) {
                    return Ok(symbol);
                }
            }
            return Err(invalid("VP8L Huffman code does not exist in table"));
        }

        let mut code = 0_u16;
        for length in 1..=self.max_code_length {
            let bit = u16::from(bits.read_bit()?);
            code |= bit << (length - 1);
            if let Some(symbol) = self.symbol_for_code(code, length) {
                return Ok(symbol);
            }
        }
        Err(invalid("VP8L Huffman code does not exist in table"))
    }

    #[inline]
    fn symbol_for_code(&self, wire_code: u16, length: u8) -> Option<usize> {
        let length = usize::from(length);
        let canonical_code = u32::from(reverse_code(wire_code, length as u8));
        let first_code = self.first_code[length];
        let code_index = canonical_code.checked_sub(first_code)?;
        let code_index = usize::try_from(code_index).ok()?;
        if code_index >= self.code_count[length] {
            return None;
        }
        self.symbols
            .get(self.first_symbol[length].checked_add(code_index)?)
            .copied()
    }

    /// Number of symbols with a nonzero code length.
    #[must_use]
    pub fn symbol_count(&self) -> usize {
        self.symbols.len()
    }

    /// Longest code in this table.
    #[must_use]
    pub const fn max_code_length(&self) -> u8 {
        self.max_code_length
    }

    /// Converts this validated table into the packed representation used by
    /// the VP8L pixel loop. The generic table remains available to parsers and
    /// other strict, low-volume callers.
    pub fn into_fast(self) -> Result<FastHuffmanTable, DecodeError> {
        FastHuffmanTable::from_validated(self)
    }
}

impl FastHuffmanTable {
    fn from_validated(table: HuffmanTable) -> Result<Self, DecodeError> {
        for &symbol in &table.symbols {
            u16::try_from(symbol).map_err(|_| invalid("VP8L fast Huffman symbol exceeds u16"))?;
        }
        if table.max_code_length == 0 {
            return Ok(Self(FastHuffmanTableInner::Single(table.symbols[0] as u16)));
        }
        if table
            .symbols
            .iter()
            .any(|&symbol| symbol > usize::from(FAST_ENTRY_VALUE_MASK))
        {
            return Ok(Self(FastHuffmanTableInner::Fallback(Box::new(table))));
        }

        let root_bits = table.max_code_length.min(FAST_ROOT_TABLE_BITS);
        let root_size = 1_usize << root_bits;
        let root_mask = u16::try_from(root_size - 1)
            .map_err(|_| invalid("VP8L fast Huffman root mask exceeds u16"))?;
        let mut secondary_bits = vec![0_u8; root_size];
        for length in root_bits + 1..=table.max_code_length {
            let length_index = usize::from(length);
            for code_index in 0..table.code_count[length_index] {
                let canonical = table.first_code[length_index] + code_index as u32;
                let wire = reverse_code(canonical as u16, length);
                let prefix = usize::from(wire) & usize::from(root_mask);
                secondary_bits[prefix] = secondary_bits[prefix].max(length - root_bits);
            }
        }

        let mut secondary_offsets = vec![0_usize; root_size];
        let mut secondary_len = 0_usize;
        for (prefix, &bits) in secondary_bits.iter().enumerate() {
            if bits == 0 {
                continue;
            }
            secondary_offsets[prefix] = secondary_len;
            secondary_len = secondary_len
                .checked_add(1_usize << bits)
                .ok_or_else(|| invalid("VP8L fast Huffman secondary size overflow"))?;
        }
        if secondary_len > usize::from(FAST_ENTRY_VALUE_MASK) + 1 {
            return Ok(Self(FastHuffmanTableInner::Fallback(Box::new(table))));
        }

        let mut root = vec![0_u16; root_size];
        for (prefix, &bits) in secondary_bits.iter().enumerate() {
            if bits == 0 {
                continue;
            }
            root[prefix] = (u16::from(root_bits + bits) << 12)
                | u16::try_from(secondary_offsets[prefix])
                    .map_err(|_| invalid("VP8L fast Huffman secondary offset exceeds u16"))?;
        }
        let mut secondary = vec![0_u16; secondary_len];

        for length in 1..=table.max_code_length {
            let length_index = usize::from(length);
            for code_index in 0..table.code_count[length_index] {
                let symbol_index = table.first_symbol[length_index] + code_index;
                let symbol = table.symbols[symbol_index] as u16;
                let canonical = table.first_code[length_index] + code_index as u32;
                let wire = reverse_code(canonical as u16, length);
                if length <= root_bits {
                    let packed = (u16::from(length) << 12) | symbol;
                    let stride = 1_usize << length;
                    for entry in root.iter_mut().skip(usize::from(wire)).step_by(stride) {
                        *entry = packed;
                    }
                    continue;
                }

                let prefix = usize::from(wire) & usize::from(root_mask);
                let table_bits = secondary_bits[prefix];
                let extra_bits = length - root_bits;
                let table_len = 1_usize << table_bits;
                let code = usize::from(wire >> root_bits);
                let stride = 1_usize << extra_bits;
                let offset = secondary_offsets[prefix];
                let packed = (symbol << 4) | u16::from(length);
                for index in (code..table_len).step_by(stride) {
                    secondary[offset + index] = packed;
                }
            }
        }

        Ok(Self(FastHuffmanTableInner::Packed {
            root_bits,
            root_mask,
            root,
            secondary,
        }))
    }

    /// Reads one symbol from the shift-register entropy reader.
    #[inline]
    pub fn decode(&self, bits: &mut ShiftedBitReader<'_, '_>) -> Result<u16, DecodeError> {
        match &self.0 {
            FastHuffmanTableInner::Single(symbol) => Ok(*symbol),
            FastHuffmanTableInner::Packed {
                root_bits,
                root_mask,
                root,
                secondary,
            } => {
                let lookahead = bits.peek_full() as u16;
                let entry = root[usize::from(lookahead & root_mask)];
                let length = (entry >> 12) as u8;
                if length <= *root_bits {
                    bits.consume(length)?;
                    return Ok(entry & FAST_ENTRY_VALUE_MASK);
                }
                decode_fast_secondary(secondary, lookahead, entry, *root_bits, bits)
            }
            FastHuffmanTableInner::Fallback(table) => table.decode_shifted(bits),
        }
    }

    /// Looks up one symbol in already-buffered, zero-padded lookahead.
    ///
    /// The returned bit count has not been consumed. Callers that combine
    /// several independent tables can therefore advance the reader once for
    /// the complete symbol group. `None` selects the strict decoder for the
    /// uncommon unpacked fallback representation.
    #[must_use]
    #[inline]
    pub fn lookup_buffered(&self, lookahead: u16) -> Option<(u16, u8)> {
        match &self.0 {
            FastHuffmanTableInner::Single(symbol) => Some((*symbol, 0)),
            FastHuffmanTableInner::Packed {
                root_bits,
                root_mask,
                root,
                secondary,
            } => {
                let entry = root[usize::from(lookahead & root_mask)];
                let length = (entry >> 12) as u8;
                if length <= *root_bits {
                    return Some((entry & FAST_ENTRY_VALUE_MASK, length));
                }

                let table_bits = length - root_bits;
                let index = usize::from(entry & FAST_ENTRY_VALUE_MASK)
                    + (usize::from(lookahead >> root_bits) & ((1_usize << table_bits) - 1));
                let entry = *secondary.get(index)?;
                Some((entry >> 4, (entry & 0x0f) as u8))
            }
            FastHuffmanTableInner::Fallback(_) => None,
        }
    }
}

#[inline(never)]
fn decode_fast_secondary(
    secondary: &[u16],
    lookahead: u16,
    root_entry: u16,
    root_bits: u8,
    bits: &mut ShiftedBitReader<'_, '_>,
) -> Result<u16, DecodeError> {
    let table_bits = (root_entry >> 12) as u8 - root_bits;
    let index = usize::from(root_entry & FAST_ENTRY_VALUE_MASK)
        + (usize::from(lookahead >> root_bits) & ((1_usize << table_bits) - 1));
    let entry = *secondary
        .get(index)
        .ok_or_else(|| invalid("VP8L fast Huffman secondary index is missing"))?;
    let length = (entry & 0x0f) as u8;
    bits.consume(length)?;
    Ok(entry >> 4)
}

impl HuffmanTable {
    fn decode_shifted(&self, bits: &mut ShiftedBitReader<'_, '_>) -> Result<u16, DecodeError> {
        if self.max_code_length == 0 {
            return u16::try_from(self.symbols[0])
                .map_err(|_| invalid("VP8L fast Huffman symbol exceeds u16"));
        }
        let mut code = 0_u16;
        for length in 1..=self.max_code_length {
            code |= (bits.read_bits(1)? as u16) << (length - 1);
            if let Some(symbol) = self.symbol_for_code(code, length) {
                return u16::try_from(symbol)
                    .map_err(|_| invalid("VP8L fast Huffman symbol exceeds u16"));
            }
        }
        Err(invalid("VP8L Huffman code does not exist in table"))
    }
}

fn invalid(context: &'static str) -> DecodeError {
    DecodeError::new(DecodeErrorKind::InvalidBitstream, None, context)
}

fn reverse_code(code: u16, length: u8) -> u16 {
    code.reverse_bits() >> (u16::BITS - u32::from(length))
}

fn fill_root_table(
    root_table: &mut [RootTableEntry; ROOT_TABLE_SIZE],
    wire_code: u16,
    length: u8,
    symbol_index: u16,
) {
    debug_assert!((1..=ROOT_TABLE_BITS).contains(&length));
    let stride = 1_usize << length;
    for entry in root_table
        .iter_mut()
        .skip(usize::from(wire_code))
        .step_by(stride)
    {
        *entry = RootTableEntry {
            symbol_index,
            bits: length,
            secondary_bits: 0,
        };
    }
}

fn fill_secondary_table(
    secondary_table: &mut [RootTableEntry],
    offset: usize,
    table_bits: u8,
    wire_code: u16,
    length: u8,
    symbol_index: u16,
) {
    let extra_bits = length - ROOT_TABLE_BITS;
    debug_assert!((1..=table_bits).contains(&extra_bits));
    let code = usize::from(wire_code >> ROOT_TABLE_BITS);
    let table_len = 1_usize << table_bits;
    let stride = 1_usize << extra_bits;
    for index in (code..table_len).step_by(stride) {
        secondary_table[offset + index] = RootTableEntry {
            symbol_index,
            bits: extra_bits,
            secondary_bits: 0,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::hint::black_box;
    use std::time::Instant;
    use webp_core::BitWriter;

    #[derive(Clone, Copy)]
    struct LinearCode {
        bits: u16,
        length: u8,
        symbol: usize,
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
                return (u32::from(reverse_code(code as u16, length)), length);
            }
            code += 1;
            previous_length = length;
        }
        panic!("requested unused symbol");
    }

    #[test]
    fn root_entries_remain_cache_compact() {
        assert_eq!(core::mem::size_of::<RootTableEntry>(), 4);
        assert_eq!(ROOT_TABLE_STORAGE_BYTES, ROOT_TABLE_SIZE * 4);
    }

    fn encoded_symbols(lengths: &[u8], symbols: &[usize]) -> Vec<u8> {
        let mut writer = BitWriter::new();
        for &symbol in symbols {
            let (code, length) = wire_code(lengths, symbol);
            writer.write_bits(code, length).unwrap();
        }
        writer.into_bytes()
    }

    // Mirrors the pre-optimization representation and lookup loop. It exists
    // only to keep a like-for-like performance baseline for the ignored
    // release benchmark below.
    fn legacy_linear_codes(lengths: &[u8]) -> Vec<LinearCode> {
        let mut counts = [0_u16; MAX_CODE_LENGTH as usize + 1];
        for &length in lengths {
            if length != 0 {
                counts[usize::from(length)] += 1;
            }
        }
        let mut next_code = [0_u16; MAX_CODE_LENGTH as usize + 1];
        let mut code = 0_u16;
        for length in 1..=usize::from(MAX_CODE_LENGTH) {
            code = (code + counts[length - 1]) << 1;
            next_code[length] = code;
        }
        let mut codes = Vec::new();
        for (symbol, &length) in lengths.iter().enumerate() {
            if length == 0 {
                continue;
            }
            let slot = &mut next_code[usize::from(length)];
            codes.push(LinearCode {
                bits: reverse_code(*slot, length),
                length,
                symbol,
            });
            *slot += 1;
        }
        codes
    }

    fn decode_legacy_linear(
        codes: &[LinearCode],
        max_code_length: u8,
        bits: &mut BitReader<'_>,
    ) -> Result<usize, DecodeError> {
        let mut code = 0_u16;
        for length in 1..=max_code_length {
            code |= u16::from(bits.read_bit()?) << (length - 1);
            if let Some(entry) = codes
                .iter()
                .find(|entry| entry.length == length && entry.bits == code)
            {
                return Ok(entry.symbol);
            }
        }
        Err(invalid("VP8L Huffman code does not exist in table"))
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
        writer.write_bits(0, 1).unwrap(); // use_length = false
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
        writer.write_bits(0, 1).unwrap(); // use_length = false
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
    fn root_lookup_does_not_require_eight_bits_at_the_end_of_input() {
        let table = HuffmanTable::from_code_lengths(&[2, 2, 2, 2]).unwrap();
        // Symbol one has canonical code 01, emitted as 10 in VP8L bit order.
        // Positioning at bit six leaves only those two code bits available.
        let mut input = BitReader::with_bit_position(&[0b1000_0000], 6).unwrap();
        assert_eq!(table.decode(&mut input), Ok(1));
        assert_eq!(input.bit_position(), 8);
    }

    #[test]
    #[ignore = "run with --release -- --ignored --nocapture to measure Huffman lookup speed"]
    fn benchmark_root_lookup_against_legacy_linear_scan() {
        // A complete 256-symbol alphabet makes the old implementation scan
        // every code for each of the first seven bits of a symbol.
        const SAMPLES: usize = 500_000;
        let lengths = vec![8; 256];
        let table = HuffmanTable::from_code_lengths(&lengths).unwrap();
        let legacy_codes = legacy_linear_codes(&lengths);
        let symbols = (0..256).cycle().take(SAMPLES).collect::<Vec<_>>();
        let encoded = encoded_symbols(&lengths, &symbols);

        let root_input = black_box(encoded.as_slice());
        let root_started = Instant::now();
        let mut root_reader = BitReader::new(root_input);
        let mut root_sum = 0_usize;
        for _ in 0..SAMPLES {
            root_sum = root_sum.wrapping_add(table.decode(&mut root_reader).unwrap());
        }
        let root_elapsed = root_started.elapsed();

        let legacy_input = black_box(encoded.as_slice());
        let legacy_started = Instant::now();
        let mut legacy_reader = BitReader::new(legacy_input);
        let mut legacy_sum = 0_usize;
        for _ in 0..SAMPLES {
            legacy_sum = legacy_sum.wrapping_add(
                decode_legacy_linear(&legacy_codes, table.max_code_length(), &mut legacy_reader)
                    .unwrap(),
            );
        }
        let legacy_elapsed = legacy_started.elapsed();

        assert_eq!(root_sum, legacy_sum);
        eprintln!("root lookup: {root_elapsed:?}; legacy linear scan: {legacy_elapsed:?}");
        assert!(
            root_elapsed < legacy_elapsed,
            "root lookup should outperform the legacy linear scan"
        );
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
    fn accepts_and_decodes_a_complete_tree_at_the_maximum_code_length() {
        // 1/2 + 1/4 + ... + 1/2^14 + 2/2^15 == 1, so this is a
        // complete, maximally unbalanced VP8L tree whose deepest codes use
        // the format's full fifteen-bit limit.
        let mut lengths = (1..MAX_CODE_LENGTH).collect::<Vec<_>>();
        lengths.extend([MAX_CODE_LENGTH, MAX_CODE_LENGTH]);
        let table = HuffmanTable::from_code_lengths(&lengths).unwrap();

        assert_eq!(table.max_code_length, MAX_CODE_LENGTH);
        assert_eq!(table.symbols.len(), lengths.len());
        let symbols = (0..lengths.len()).collect::<Vec<_>>();
        let encoded = encoded_symbols(&lengths, &symbols);
        let mut reader = BitReader::new(&encoded);
        for symbol in symbols {
            assert_eq!(table.decode(&mut reader), Ok(symbol));
        }
    }

    #[test]
    fn fast_table_decodes_root_and_secondary_codes() {
        let mut lengths = (1..MAX_CODE_LENGTH).collect::<Vec<_>>();
        lengths.extend([MAX_CODE_LENGTH, MAX_CODE_LENGTH]);
        let symbols = (0..lengths.len()).collect::<Vec<_>>();
        let encoded = encoded_symbols(&lengths, &symbols);
        let table = HuffmanTable::from_code_lengths(&lengths)
            .unwrap()
            .into_fast()
            .unwrap();
        let mut owner = BitReader::new(&encoded);
        {
            let mut reader = owner.shifted();
            for symbol in symbols {
                reader.fill();
                assert_eq!(table.decode(&mut reader), Ok(symbol as u16));
            }
        }
        assert_eq!(
            owner.bit_position(),
            lengths.iter().map(|&v| usize::from(v)).sum()
        );
    }

    #[test]
    fn fast_table_tail_error_preserves_the_consumed_cursor() {
        let table = HuffmanTable::from_code_lengths(&[1, 2, 3, 3])
            .unwrap()
            .into_fast()
            .unwrap();
        let mut owner = BitReader::with_bit_position(&[0b1100_0000], 6).unwrap();
        {
            let mut reader = owner.shifted();
            reader.fill();
            assert_eq!(
                table.decode(&mut reader).unwrap_err().kind(),
                DecodeErrorKind::UnexpectedEof
            );
        }
        assert_eq!(owner.bit_position(), 6);
    }

    #[test]
    fn fast_table_falls_back_for_symbols_outside_packed_entry_range() {
        let mut lengths = vec![0; usize::from(FAST_ENTRY_VALUE_MASK) + 2];
        lengths[0] = 1;
        lengths[usize::from(FAST_ENTRY_VALUE_MASK) + 1] = 1;
        let table = HuffmanTable::from_code_lengths(&lengths)
            .unwrap()
            .into_fast()
            .unwrap();
        let mut owner = BitReader::new(&[0b0000_0010]);
        let mut reader = owner.shifted();
        reader.fill();
        assert_eq!(table.decode(&mut reader), Ok(0));
        assert_eq!(table.decode(&mut reader), Ok(FAST_ENTRY_VALUE_MASK + 1));
    }

    #[test]
    fn fast_table_handle_remains_cache_compact() {
        assert!(core::mem::size_of::<FastHuffmanTable>() <= 64);
    }

    #[test]
    fn buffered_lookup_reports_symbol_and_length_without_consuming() {
        let lengths = vec![8; 256];
        let table = HuffmanTable::from_code_lengths(&lengths)
            .unwrap()
            .into_fast()
            .unwrap();
        for symbol in 0_u16..=255 {
            let wire = symbol.reverse_bits() >> 8;
            assert_eq!(table.lookup_buffered(wire), Some((symbol, 8)));
        }

        let single = HuffmanTable::from_code_lengths(&[0, 1, 0])
            .unwrap()
            .into_fast()
            .unwrap();
        assert_eq!(single.lookup_buffered(u16::MAX), Some((1, 0)));
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
    fn huffman_code_selects_the_simple_representation() {
        let mut stream = BitWriter::new();
        stream.write_bits(1, 1).unwrap(); // simple_code_flag
        stream.write_bits(0, 1).unwrap(); // one symbol
        stream.write_bits(0, 1).unwrap(); // first symbol uses one bit
        stream.write_bits(1, 1).unwrap(); // symbol one

        let mut input = BitReader::new(stream.as_bytes());
        let table = read_huffman_code(&mut input, 2).unwrap();
        assert_eq!(table.symbol_count(), 1);
        assert_eq!(table.decode(&mut input), Ok(1));
        assert_eq!(input.bit_position(), stream.bit_len());
    }

    #[test]
    fn huffman_code_selects_the_normal_representation() {
        let lengths = [1, 1];
        let mut stream = BitWriter::new();
        stream.write_bits(0, 1).unwrap(); // simple_code_flag
        write_normal_header(&mut stream, true);
        stream.write_bits(0, 1).unwrap(); // use_length = false
        write_code_length_symbol(&mut stream, true, 1);
        write_code_length_symbol(&mut stream, true, 1);
        let (code, width) = wire_code(&lengths, 1);
        stream.write_bits(code, width).unwrap();

        let mut input = BitReader::new(stream.as_bytes());
        let table = read_huffman_code(&mut input, lengths.len()).unwrap();
        assert_eq!(table.symbol_count(), lengths.len());
        assert_eq!(table.decode(&mut input), Ok(1));
        assert_eq!(input.bit_position(), stream.bit_len());
    }

    #[test]
    fn huffman_code_requires_a_complete_simple_code_flag() {
        let error = read_huffman_code(&mut BitReader::new(&[]), 2).unwrap_err();
        assert_eq!(error.kind(), DecodeErrorKind::UnexpectedEof);

        // Position the reader so exactly one flag bit is available.  Its set
        // value dispatches to the simple parser, which must then report the
        // missing representation rather than silently accepting the flag.
        let mut flag = BitWriter::new();
        flag.write_bits(1, 1).unwrap();
        let (prefix, start) = simple_prefix(&flag, 1);
        let mut input = BitReader::with_bit_position(&prefix, start).unwrap();
        let error = read_huffman_code(&mut input, 2).unwrap_err();
        assert_eq!(error.kind(), DecodeErrorKind::UnexpectedEof);
        assert_eq!(input.bit_position(), start + 1);
    }

    #[test]
    fn huffman_code_preserves_normal_header_errors() {
        let mut stream = BitWriter::new();
        stream.write_bits(0, 1).unwrap(); // simple_code_flag
        stream.write_bits(0, 4).unwrap();
        for length in [1, 1, 1, 0] {
            stream.write_bits(length, 3).unwrap();
        }

        let error = read_huffman_code(&mut BitReader::new(stream.as_bytes()), 1).unwrap_err();
        assert_eq!(error.kind(), DecodeErrorKind::InvalidBitstream);
        assert_eq!(error.context(), "VP8L Huffman tree is oversubscribed");
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
    fn normal_code_lengths_support_a_shortened_max_symbol_prefix() {
        let mut stream = BitWriter::new();
        stream.write_bits(0, 1).unwrap(); // simple_code_flag
        write_normal_header(&mut stream, true);
        stream.write_bits(1, 1).unwrap(); // use_length
        stream.write_bits(0, 3).unwrap(); // length_nbits = 2
        stream.write_bits(0, 2).unwrap(); // max_symbol = 2
        write_code_length_symbol(&mut stream, true, 1);
        write_code_length_symbol(&mut stream, true, 1);
        stream.write_bits(1, 1).unwrap(); // data: symbol one

        let mut input = BitReader::new(stream.as_bytes());
        let table = read_huffman_code(&mut input, 4).unwrap();
        assert_eq!(table.symbol_count(), 2);
        assert_eq!(table.decode(&mut input), Ok(1));
        assert_eq!(input.bit_position(), stream.bit_len());
    }

    #[test]
    fn shortened_stream_counts_repeat_as_one_code_length_symbol() {
        let mut stream = BitWriter::new();
        write_normal_header(&mut stream, true);
        stream.write_bits(1, 1).unwrap(); // use_length
        stream.write_bits(0, 3).unwrap(); // length_nbits = 2
        stream.write_bits(0, 2).unwrap(); // read two code-length symbols
        write_code_length_symbol(&mut stream, true, 1);
        write_code_length_symbol(&mut stream, true, 16);
        stream.write_bits(0, 2).unwrap(); // repeat previous length three times

        assert_eq!(
            read_normal_code_lengths(&mut BitReader::new(stream.as_bytes()), 6).unwrap(),
            [1, 1, 1, 1, 0, 0]
        );
    }

    #[test]
    fn normal_code_lengths_accept_every_max_symbol_width() {
        for selector in 0..=7_u32 {
            let mut stream = BitWriter::new();
            write_normal_header(&mut stream, true);
            stream.write_bits(1, 1).unwrap(); // use_length
            stream.write_bits(selector, 3).unwrap();
            stream.write_bits(0, (2 + 2 * selector) as u8).unwrap(); // max_symbol = 2
            write_code_length_symbol(&mut stream, true, 1);
            write_code_length_symbol(&mut stream, true, 1);

            assert_eq!(
                read_normal_code_lengths(&mut BitReader::new(stream.as_bytes()), 2).unwrap(),
                [1, 1]
            );
        }
    }

    #[test]
    fn normal_code_lengths_reject_a_max_symbol_past_the_alphabet() {
        let mut stream = BitWriter::new();
        write_normal_header(&mut stream, false);
        stream.write_bits(1, 1).unwrap(); // use_length
        stream.write_bits(0, 3).unwrap(); // length_nbits = 2
        stream.write_bits(3, 2).unwrap(); // max_symbol = 5

        let error =
            read_normal_code_lengths(&mut BitReader::new(stream.as_bytes()), 4).unwrap_err();
        assert_eq!(error.kind(), DecodeErrorKind::InvalidBitstream);
        assert_eq!(
            error.context(),
            "VP8L code-length max symbol exceeds alphabet"
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
        over.write_bits(0, 1).unwrap(); // use_length = false
        let error = read_normal_code_lengths(&mut BitReader::new(over.as_bytes()), 1).unwrap_err();
        assert_eq!(error.context(), "VP8L Huffman tree is oversubscribed");

        let mut under = BitWriter::new();
        under.write_bits(0, 4).unwrap();
        for length in [2, 2, 0, 0] {
            under.write_bits(length, 3).unwrap();
        }
        under.write_bits(0, 1).unwrap(); // use_length = false
        let error = read_normal_code_lengths(&mut BitReader::new(under.as_bytes()), 1).unwrap_err();
        assert_eq!(error.context(), "VP8L Huffman tree is incomplete");
    }

    #[test]
    fn normal_code_lengths_reject_an_empty_target_alphabet() {
        let error = read_normal_code_lengths(&mut BitReader::new(&[]), 0).unwrap_err();
        assert_eq!(error.kind(), DecodeErrorKind::InvalidBitstream);
    }
}
