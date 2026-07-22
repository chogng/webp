//! Frequency-derived Huffman tables for headerless VP8L alpha streams.

use crate::AlphaEncodeError;
use webp_core::BitWriter;

const MAX_CODE_LENGTH: usize = 15;
const MAX_CODE_LENGTH_CODE_LENGTH: usize = 7;
const CODE_LENGTH_CODE_ORDER: [usize; 19] = [
    17, 18, 0, 1, 2, 3, 4, 5, 16, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15,
];

pub(super) struct EncodingTable {
    codes: Vec<(u32, u8)>,
}

pub(super) fn write_adaptive_table(
    writer: &mut BitWriter,
    frequencies: &[u32],
) -> Result<EncodingTable, AlphaEncodeError> {
    let lengths = code_lengths(frequencies)?;
    let mut symbols = lengths
        .iter()
        .copied()
        .enumerate()
        .filter_map(|(symbol, length)| (length != 0).then_some(symbol));
    let first = symbols.next().ok_or(AlphaEncodeError::SizeOverflow)?;
    if symbols.next().is_none()
        && let Ok(symbol) = u8::try_from(first)
    {
        write_simple_table(writer, symbol)?;
        return canonical_table(&lengths);
    }
    write_normal_table(writer, &lengths)?;
    canonical_table(&lengths)
}

pub(super) fn write_table_symbol(
    writer: &mut BitWriter,
    table: &EncodingTable,
    symbol: usize,
) -> Result<(), AlphaEncodeError> {
    let (code, width) = table
        .codes
        .get(symbol)
        .copied()
        .ok_or(AlphaEncodeError::SizeOverflow)?;
    write_canonical_symbol(writer, code, width)
}

pub(super) fn table_wire_symbol(
    table: &EncodingTable,
    symbol: usize,
) -> Result<(u32, u8), AlphaEncodeError> {
    let (code, width) = table
        .codes
        .get(symbol)
        .copied()
        .ok_or(AlphaEncodeError::SizeOverflow)?;
    if width == 0 {
        return Ok((0, 0));
    }
    let wire_code = code.reverse_bits() >> (u32::BITS - u32::from(width));
    Ok((wire_code, width))
}

pub(super) fn write_simple_table(
    writer: &mut BitWriter,
    symbol: u8,
) -> Result<(), AlphaEncodeError> {
    write_bits(writer, 1, 1)?;
    write_bits(writer, 0, 1)?;
    write_bits(writer, 1, 1)?;
    write_bits(writer, u32::from(symbol), 8)
}

fn code_lengths(frequencies: &[u32]) -> Result<Vec<u8>, AlphaEncodeError> {
    code_lengths_with_limit(frequencies, MAX_CODE_LENGTH)
}

fn code_lengths_with_limit(
    frequencies: &[u32],
    maximum_length: usize,
) -> Result<Vec<u8>, AlphaEncodeError> {
    let mut ranked = frequencies
        .iter()
        .copied()
        .enumerate()
        .filter_map(|(symbol, frequency)| (frequency != 0).then_some((frequency, symbol)))
        .collect::<Vec<_>>();
    if ranked.is_empty() {
        ranked.push((1, 0));
    }
    ranked.sort_unstable_by(|left, right| right.0.cmp(&left.0).then_with(|| left.1.cmp(&right.1)));
    if ranked.len() == 1 {
        let mut lengths = zero_lengths(frequencies.len())?;
        lengths[ranked[0].1] = 1;
        return Ok(lengths);
    }

    let mut nodes = Vec::new();
    nodes
        .try_reserve_exact(ranked.len().saturating_mul(2).saturating_sub(1))
        .map_err(|_| AlphaEncodeError::AllocationFailed)?;
    for &(frequency, symbol) in &ranked {
        nodes.push(HuffmanNode {
            frequency: u64::from(frequency),
            minimum_symbol: symbol,
            leaf: Some(symbol),
            children: None,
        });
    }
    let mut active = Vec::new();
    active
        .try_reserve_exact(ranked.len())
        .map_err(|_| AlphaEncodeError::AllocationFailed)?;
    active.extend(0..ranked.len());
    while active.len() > 1 {
        active.sort_unstable_by(|&left, &right| {
            nodes[right]
                .frequency
                .cmp(&nodes[left].frequency)
                .then_with(|| nodes[right].minimum_symbol.cmp(&nodes[left].minimum_symbol))
        });
        let left = active.pop().ok_or(AlphaEncodeError::SizeOverflow)?;
        let right = active.pop().ok_or(AlphaEncodeError::SizeOverflow)?;
        let node = HuffmanNode {
            frequency: nodes[left]
                .frequency
                .checked_add(nodes[right].frequency)
                .ok_or(AlphaEncodeError::SizeOverflow)?,
            minimum_symbol: nodes[left].minimum_symbol.min(nodes[right].minimum_symbol),
            leaf: None,
            children: Some((left, right)),
        };
        nodes.push(node);
        active.push(nodes.len() - 1);
    }

    let mut lengths = zero_lengths(frequencies.len())?;
    let mut stack = Vec::new();
    stack
        .try_reserve_exact(nodes.len())
        .map_err(|_| AlphaEncodeError::AllocationFailed)?;
    stack.push((active[0], 0_usize));
    let mut too_deep = false;
    while let Some((index, depth)) = stack.pop() {
        let node = &nodes[index];
        if let Some(symbol) = node.leaf {
            too_deep |= depth > maximum_length;
            lengths[symbol] = depth.min(maximum_length) as u8;
        } else if let Some((left, right)) = node.children {
            stack.push((right, depth + 1));
            stack.push((left, depth + 1));
        }
    }
    if too_deep {
        balanced_lengths(&ranked, frequencies.len())
    } else {
        Ok(lengths)
    }
}

fn balanced_lengths(
    ranked: &[(u32, usize)],
    alphabet_size: usize,
) -> Result<Vec<u8>, AlphaEncodeError> {
    let mut lengths = zero_lengths(alphabet_size)?;
    let floor_log = usize::BITS - 1 - ranked.len().leading_zeros();
    let base = 1_usize << floor_log;
    let short_count = base
        .checked_mul(2)
        .and_then(|count| count.checked_sub(ranked.len()))
        .ok_or(AlphaEncodeError::SizeOverflow)?;
    for (rank, (_, symbol)) in ranked.iter().enumerate() {
        lengths[*symbol] = if rank < short_count {
            floor_log as u8
        } else {
            floor_log as u8 + 1
        };
    }
    Ok(lengths)
}

fn zero_lengths(alphabet_size: usize) -> Result<Vec<u8>, AlphaEncodeError> {
    let mut lengths = Vec::new();
    lengths
        .try_reserve_exact(alphabet_size)
        .map_err(|_| AlphaEncodeError::AllocationFailed)?;
    lengths.resize(alphabet_size, 0);
    Ok(lengths)
}

fn write_normal_table(writer: &mut BitWriter, lengths: &[u8]) -> Result<(), AlphaEncodeError> {
    let tokens = encode_code_lengths(lengths)?;
    let mut frequencies = [0_u32; 19];
    for token in &tokens {
        frequencies[token.symbol] = frequencies[token.symbol]
            .checked_add(1)
            .ok_or(AlphaEncodeError::SizeOverflow)?;
    }
    let code_length_lengths = code_lengths_with_limit(&frequencies, MAX_CODE_LENGTH_CODE_LENGTH)?;
    let code_count = CODE_LENGTH_CODE_ORDER
        .iter()
        .rposition(|&symbol| code_length_lengths[symbol] != 0)
        .map(|index| (index + 1).max(4))
        .ok_or(AlphaEncodeError::SizeOverflow)?;

    write_bits(writer, 0, 1)?;
    write_bits(
        writer,
        u32::try_from(code_count - 4).map_err(|_| AlphaEncodeError::SizeOverflow)?,
        4,
    )?;
    for &symbol in CODE_LENGTH_CODE_ORDER.iter().take(code_count) {
        write_bits(writer, u32::from(code_length_lengths[symbol]), 3)?;
    }
    write_bits(writer, 0, 1)?;
    let table = canonical_table(&code_length_lengths)?;
    for token in tokens {
        write_table_symbol(writer, &table, token.symbol)?;
        write_bits(writer, token.extra, token.extra_bits)?;
    }
    Ok(())
}

fn encode_code_lengths(lengths: &[u8]) -> Result<Vec<CodeLengthToken>, AlphaEncodeError> {
    let mut tokens = Vec::new();
    tokens
        .try_reserve_exact(lengths.len())
        .map_err(|_| AlphaEncodeError::AllocationFailed)?;
    let mut index = 0_usize;
    while index < lengths.len() {
        let value = lengths[index];
        let mut run = 1_usize;
        while index + run < lengths.len() && lengths[index + run] == value {
            run += 1;
        }
        if value == 0 {
            emit_zero_run(&mut tokens, run)?;
        } else {
            tokens.push(CodeLengthToken::value(value));
            emit_previous_run(&mut tokens, value, run - 1)?;
        }
        index += run;
    }
    Ok(tokens)
}

fn emit_zero_run(
    tokens: &mut Vec<CodeLengthToken>,
    mut count: usize,
) -> Result<(), AlphaEncodeError> {
    while count >= 11 {
        let repeated = count.min(138);
        tokens.push(CodeLengthToken::repeat(18, repeated - 11, 7)?);
        count -= repeated;
    }
    if count >= 3 {
        let repeated = count.min(10);
        tokens.push(CodeLengthToken::repeat(17, repeated - 3, 3)?);
        count -= repeated;
    }
    tokens.extend((0..count).map(|_| CodeLengthToken::value(0)));
    Ok(())
}

fn emit_previous_run(
    tokens: &mut Vec<CodeLengthToken>,
    value: u8,
    mut count: usize,
) -> Result<(), AlphaEncodeError> {
    while count >= 3 {
        let repeated = count.min(6);
        tokens.push(CodeLengthToken::repeat(16, repeated - 3, 2)?);
        count -= repeated;
    }
    tokens.extend((0..count).map(|_| CodeLengthToken::value(value)));
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CodeLengthToken {
    symbol: usize,
    extra: u32,
    extra_bits: u8,
}

impl CodeLengthToken {
    const fn value(value: u8) -> Self {
        Self {
            symbol: value as usize,
            extra: 0,
            extra_bits: 0,
        }
    }

    fn repeat(symbol: usize, extra: usize, extra_bits: u8) -> Result<Self, AlphaEncodeError> {
        Ok(Self {
            symbol,
            extra: u32::try_from(extra).map_err(|_| AlphaEncodeError::SizeOverflow)?,
            extra_bits,
        })
    }
}

fn canonical_table(lengths: &[u8]) -> Result<EncodingTable, AlphaEncodeError> {
    let mut symbols = lengths
        .iter()
        .copied()
        .enumerate()
        .filter_map(|(symbol, length)| (length != 0).then_some((length, symbol)))
        .collect::<Vec<_>>();
    symbols.sort_unstable();
    let mut codes = zero_codes(lengths.len())?;
    if symbols.len() == 1 {
        codes[symbols[0].1] = (0, 0);
        return Ok(EncodingTable { codes });
    }
    let mut code = 0_u32;
    let mut previous_length = 0_u8;
    for (length, symbol) in symbols {
        code <<= u32::from(length - previous_length);
        codes[symbol] = (code, length);
        code = code.checked_add(1).ok_or(AlphaEncodeError::SizeOverflow)?;
        previous_length = length;
    }
    Ok(EncodingTable { codes })
}

fn zero_codes(alphabet_size: usize) -> Result<Vec<(u32, u8)>, AlphaEncodeError> {
    let mut codes = Vec::new();
    codes
        .try_reserve_exact(alphabet_size)
        .map_err(|_| AlphaEncodeError::AllocationFailed)?;
    codes.resize(alphabet_size, (0, 0));
    Ok(codes)
}

#[cfg(test)]
pub(super) fn table_from_codes_for_test(codes: Vec<(u32, u8)>) -> EncodingTable {
    EncodingTable { codes }
}

fn write_canonical_symbol(
    writer: &mut BitWriter,
    canonical_code: u32,
    width: u8,
) -> Result<(), AlphaEncodeError> {
    if width == 0 {
        return Ok(());
    }
    let wire_code = canonical_code.reverse_bits() >> (u32::BITS - u32::from(width));
    write_bits(writer, wire_code, width)
}

fn write_bits(writer: &mut BitWriter, value: u32, count: u8) -> Result<(), AlphaEncodeError> {
    writer
        .write_bits(value, count)
        .map_err(|_| AlphaEncodeError::AllocationFailed)
}

struct HuffmanNode {
    frequency: u64,
    minimum_symbol: usize,
    leaf: Option<usize>,
    children: Option<(usize, usize)>,
}

#[cfg(test)]
#[path = "encode_huffman_tests.rs"]
mod tests;
