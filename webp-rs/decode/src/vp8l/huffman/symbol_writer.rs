//! Shared VP8L canonical-symbol wire writing.

use crate::BitWriter;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WireWriteError {
    SizeOverflow,
    AllocationFailed,
}

pub struct EncodingTable {
    pub codes: Vec<(u32, u8)>,
}

pub fn canonical_table(lengths: &[u8]) -> Result<EncodingTable, WireWriteError> {
    let mut symbols = lengths
        .iter()
        .copied()
        .enumerate()
        .filter_map(|(symbol, length)| (length != 0).then_some((length, symbol)))
        .collect::<Vec<_>>();
    symbols.sort_unstable();
    let mut codes = Vec::new();
    codes
        .try_reserve_exact(lengths.len())
        .map_err(|_| WireWriteError::AllocationFailed)?;
    codes.resize(lengths.len(), (0, 0));
    if symbols.len() == 1 {
        codes[symbols[0].1] = (0, 0);
        return Ok(EncodingTable { codes });
    }
    let mut code = 0_u32;
    let mut previous_length = 0_u8;
    for (length, symbol) in symbols {
        code <<= u32::from(length - previous_length);
        codes[symbol] = (code, length);
        code = code.checked_add(1).ok_or(WireWriteError::SizeOverflow)?;
        previous_length = length;
    }
    Ok(EncodingTable { codes })
}

pub fn write_table_symbol(
    writer: &mut BitWriter,
    table: &EncodingTable,
    symbol: usize,
) -> Result<(), WireWriteError> {
    let (code, width) = table
        .codes
        .get(symbol)
        .copied()
        .ok_or(WireWriteError::SizeOverflow)?;
    write_canonical_symbol(writer, code, width)
}

pub fn table_wire_symbol(
    table: &EncodingTable,
    symbol: usize,
) -> Result<(u32, u8), WireWriteError> {
    let (code, width) = table
        .codes
        .get(symbol)
        .copied()
        .ok_or(WireWriteError::SizeOverflow)?;
    if width == 0 {
        return Ok((0, 0));
    }
    Ok((code.reverse_bits() >> (u32::BITS - u32::from(width)), width))
}

#[doc(hidden)]
pub fn table_from_codes_for_test(codes: Vec<(u32, u8)>) -> EncodingTable {
    EncodingTable { codes }
}

pub fn write_simple_table(writer: &mut BitWriter, symbol: u8) -> Result<(), WireWriteError> {
    write_bits(writer, 1, 1)?;
    write_bits(writer, 0, 1)?;
    write_bits(writer, 1, 1)?;
    write_bits(writer, u32::from(symbol), 8)
}

pub(crate) fn write_canonical_symbol(
    writer: &mut BitWriter,
    canonical_code: u32,
    width: u8,
) -> Result<(), WireWriteError> {
    if width == 0 {
        return Ok(());
    }
    let wire_code = canonical_code.reverse_bits() >> (u32::BITS - u32::from(width));
    write_bits(writer, wire_code, width)
}

fn write_bits(writer: &mut BitWriter, value: u32, count: u8) -> Result<(), WireWriteError> {
    writer
        .write_bits(value, count)
        .map_err(|_| WireWriteError::AllocationFailed)
}

impl From<WireWriteError> for crate::EncodeError {
    fn from(error: WireWriteError) -> Self {
        match error {
            WireWriteError::SizeOverflow => Self::output_size_overflow(),
            WireWriteError::AllocationFailed => Self::allocation_failed(),
        }
    }
}
