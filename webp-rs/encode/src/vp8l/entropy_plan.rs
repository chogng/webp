//! Candidate-independent Huffman preparation and exact VP8L bit accounting.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use super::token_stream::{EntropyFrequencies, MAX_ENCODER_COLOR_CACHE_BITS, TokenStatistics};
use super::{
    BitWriter, CHANNEL_ALPHABET_SIZE, EncodeError, EntropyTables, canonical_table,
    prepare_adaptive_table, write_bits, write_normal_table,
};

const NORMAL_TABLE_FIXED_BITS: usize = 63;

#[derive(Clone, Copy)]
enum TableHeader {
    Normal,
    Simple { first: u8, second: Option<u8> },
}

pub(crate) struct EntropyPlan {
    green_lengths: Vec<u8>,
    red_lengths: Vec<u8>,
    blue_lengths: Vec<u8>,
    alpha_lengths: Vec<u8>,
    distance_lengths: Vec<u8>,
    headers: [TableHeader; 5],
    tables: EntropyTables,
    table_bits: usize,
    token_bits: usize,
    secondary_lookups: usize,
}

impl EntropyPlan {
    pub(crate) fn build_for_stream(statistics: &TokenStatistics) -> Result<Self, EncodeError> {
        let plan = Self::build(statistics.frequencies())?;
        Self::validate_stream(plan, statistics)
    }

    pub(crate) fn build_compact_for_stream(
        statistics: &TokenStatistics,
    ) -> Result<Self, EncodeError> {
        let plan = Self::build_compact(statistics.frequencies())?;
        Self::validate_stream(plan, statistics)
    }

    fn validate_stream(plan: Self, statistics: &TokenStatistics) -> Result<Self, EncodeError> {
        let census = statistics.census();
        let distance_symbols = statistics
            .frequencies()
            .distance()
            .iter()
            .try_fold(0_usize, |total, &frequency| {
                total.checked_add(frequency as usize)
            })
            .ok_or_else(EncodeError::output_size_overflow)?;
        if census.copy_tokens() != census.distance_symbols()
            || census.distance_symbols() != distance_symbols
        {
            return Err(EncodeError::output_size_overflow());
        }
        Ok(plan)
    }

    pub(crate) fn build(frequencies: &EntropyFrequencies) -> Result<Self, EncodeError> {
        Self::build_internal(frequencies, false)
    }

    pub(crate) fn build_compact(frequencies: &EntropyFrequencies) -> Result<Self, EncodeError> {
        Self::build_internal(frequencies, true)
    }

    fn build_internal(
        frequencies: &EntropyFrequencies,
        compact: bool,
    ) -> Result<Self, EncodeError> {
        let prepare = |values: &[u32]| {
            if compact {
                prepare_compact_table(values)
            } else {
                let (lengths, table) = prepare_adaptive_table(values)?;
                Ok((lengths, table, TableHeader::Normal))
            }
        };
        let (green_lengths, green, green_header) = prepare(frequencies.green())?;
        let (red_lengths, red, red_header) = prepare(frequencies.red())?;
        let (blue_lengths, blue, blue_header) = prepare(frequencies.blue())?;
        let (alpha_lengths, alpha, alpha_header) = prepare(frequencies.alpha())?;
        let (distance_lengths, distance, distance_header) = prepare(frequencies.distance())?;
        let headers = [
            green_header,
            red_header,
            blue_header,
            alpha_header,
            distance_header,
        ];
        let tables = EntropyTables {
            green,
            red,
            blue,
            alpha,
            distance,
        };

        let mut table_bits = 0_usize;
        for (lengths, header) in [
            (&green_lengths, headers[0]),
            (&red_lengths, headers[1]),
            (&blue_lengths, headers[2]),
            (&alpha_lengths, headers[3]),
            (&distance_lengths, headers[4]),
        ] {
            table_bits = table_bits
                .checked_add(table_header_bits(lengths.len(), header)?)
                .ok_or_else(EncodeError::output_size_overflow)?;
        }

        let mut token_bits = 0_usize;
        for (frequency, (_, width)) in frequencies.green().iter().zip(&tables.green.codes) {
            token_bits = add_weighted_width(token_bits, *frequency, *width)?;
        }
        for (frequencies, table) in [
            (&frequencies.red()[..], &tables.red),
            (&frequencies.blue()[..], &tables.blue),
            (&frequencies.alpha()[..], &tables.alpha),
            (&frequencies.distance()[..], &tables.distance),
        ] {
            for (frequency, (_, width)) in frequencies.iter().zip(&table.codes) {
                token_bits = add_weighted_width(token_bits, *frequency, *width)?;
            }
        }

        let mut green_copy_count = 0_u64;
        for prefix in 0..24 {
            let frequency = frequencies.green()[CHANNEL_ALPHABET_SIZE + prefix];
            green_copy_count = green_copy_count
                .checked_add(u64::from(frequency))
                .ok_or_else(EncodeError::output_size_overflow)?;
            let extra_bits = if prefix < 4 { 0 } else { (prefix - 2) >> 1 };
            token_bits = add_weighted_width(
                token_bits,
                frequency,
                u8::try_from(extra_bits).map_err(|_| EncodeError::output_size_overflow())?,
            )?;
        }
        let mut distance_copy_count = 0_u64;
        for (prefix, &frequency) in frequencies.distance().iter().enumerate() {
            distance_copy_count = distance_copy_count
                .checked_add(u64::from(frequency))
                .ok_or_else(EncodeError::output_size_overflow)?;
            let extra_bits = if prefix < 4 { 0 } else { (prefix - 2) >> 1 };
            token_bits = add_weighted_width(
                token_bits,
                frequency,
                u8::try_from(extra_bits).map_err(|_| EncodeError::output_size_overflow())?,
            )?;
        }
        if green_copy_count != distance_copy_count {
            return Err(EncodeError::output_size_overflow());
        }

        let mut secondary_lookups = 0_usize;
        for (frequencies, lengths) in [
            (frequencies.green(), &green_lengths[..]),
            (&frequencies.red()[..], &red_lengths[..]),
            (&frequencies.blue()[..], &blue_lengths[..]),
            (&frequencies.alpha()[..], &alpha_lengths[..]),
            (&frequencies.distance()[..], &distance_lengths[..]),
        ] {
            for (&frequency, &length) in frequencies.iter().zip(lengths) {
                if length > 10 {
                    secondary_lookups = secondary_lookups
                        .checked_add(frequency as usize)
                        .ok_or_else(EncodeError::output_size_overflow)?;
                }
            }
        }

        Ok(Self {
            green_lengths,
            red_lengths,
            blue_lengths,
            alpha_lengths,
            distance_lengths,
            headers,
            tables,
            table_bits,
            token_bits,
            secondary_lookups,
        })
    }

    pub(crate) fn write_main_prefix(
        &self,
        bits: &mut BitWriter,
        color_cache_bits: u8,
    ) -> Result<(), EncodeError> {
        if color_cache_bits > MAX_ENCODER_COLOR_CACHE_BITS {
            return Err(EncodeError::output_size_overflow());
        }
        write_bits(bits, u32::from(color_cache_bits != 0), 1)?;
        if color_cache_bits != 0 {
            write_bits(bits, u32::from(color_cache_bits), 4)?;
        }
        write_bits(bits, 0, 1)?;
        self.write_tables(bits)
    }

    pub(crate) fn write_tables(&self, bits: &mut BitWriter) -> Result<(), EncodeError> {
        for (lengths, header) in [
            (&self.green_lengths, self.headers[0]),
            (&self.red_lengths, self.headers[1]),
            (&self.blue_lengths, self.headers[2]),
            (&self.alpha_lengths, self.headers[3]),
            (&self.distance_lengths, self.headers[4]),
        ] {
            match header {
                TableHeader::Normal => write_normal_table(bits, lengths)?,
                TableHeader::Simple { first, second } => {
                    write_simple_header(bits, first, second)?;
                }
            }
        }
        Ok(())
    }

    pub(crate) const fn tables(&self) -> &EntropyTables {
        &self.tables
    }

    pub(crate) const fn token_bits(&self) -> usize {
        self.token_bits
    }

    pub(crate) const fn secondary_lookups(&self) -> usize {
        self.secondary_lookups
    }

    pub(crate) fn encoded_bits(&self) -> Result<usize, EncodeError> {
        self.table_bits
            .checked_add(self.token_bits)
            .ok_or_else(EncodeError::output_size_overflow)
    }

    pub(crate) fn main_bits(&self, color_cache_bits: u8) -> Result<usize, EncodeError> {
        if color_cache_bits > MAX_ENCODER_COLOR_CACHE_BITS {
            return Err(EncodeError::output_size_overflow());
        }
        let header_bits = 2_usize + usize::from(color_cache_bits != 0) * 4;
        header_bits
            .checked_add(self.encoded_bits()?)
            .ok_or_else(EncodeError::output_size_overflow)
    }
}

pub(crate) fn payload_bytes(payload_bits: usize) -> Result<usize, EncodeError> {
    payload_bits
        .checked_add(7)
        .map(|bits| bits / 8)
        .ok_or_else(EncodeError::output_size_overflow)
}

pub(crate) fn riff_bytes(payload_bits: usize) -> Result<usize, EncodeError> {
    let payload_bytes = payload_bytes(payload_bits)?;
    let padded_payload_bytes = payload_bytes
        .checked_add(payload_bytes & 1)
        .ok_or_else(EncodeError::output_size_overflow)?;
    let riff_size = 12_usize
        .checked_add(padded_payload_bytes)
        .ok_or_else(EncodeError::output_size_overflow)?;
    u32::try_from(riff_size).map_err(|_| EncodeError::output_size_overflow())?;
    riff_size
        .checked_add(8)
        .ok_or_else(EncodeError::output_size_overflow)
}

fn table_header_bits(alphabet_len: usize, header: TableHeader) -> Result<usize, EncodeError> {
    match header {
        TableHeader::Normal => alphabet_len
            .checked_mul(4)
            .and_then(|bits| bits.checked_add(NORMAL_TABLE_FIXED_BITS))
            .ok_or_else(EncodeError::output_size_overflow),
        TableHeader::Simple { first, second } => {
            Ok(3 + if first <= 1 { 1 } else { 8 } + usize::from(second.is_some()) * 8)
        }
    }
}

fn add_weighted_width(total: usize, frequency: u32, width: u8) -> Result<usize, EncodeError> {
    usize::try_from(frequency)
        .ok()
        .and_then(|frequency| frequency.checked_mul(usize::from(width)))
        .and_then(|bits| total.checked_add(bits))
        .ok_or_else(EncodeError::output_size_overflow)
}

fn prepare_compact_table(
    frequencies: &[u32],
) -> Result<(Vec<u8>, super::EncodingTable, TableHeader), EncodeError> {
    let symbols = frequencies
        .iter()
        .enumerate()
        .filter_map(|(symbol, &frequency)| (frequency != 0).then_some(symbol))
        .collect::<Vec<_>>();
    if !symbols.is_empty()
        && symbols.len() <= 2
        && symbols.iter().all(|&symbol| symbol <= usize::from(u8::MAX))
    {
        let first = symbols[0] as u8;
        let second = symbols.get(1).map(|&symbol| symbol as u8);
        let mut lengths = vec![0_u8; frequencies.len()];
        lengths[usize::from(first)] = 1;
        if let Some(second) = second {
            lengths[usize::from(first)] = 1;
            lengths[usize::from(second)] = 1;
        }
        let table = canonical_table(&lengths)?;
        return Ok((lengths, table, TableHeader::Simple { first, second }));
    }

    let lengths = huffman_lengths(frequencies)?;
    let table = canonical_table(&lengths)?;
    Ok((lengths, table, TableHeader::Normal))
}

fn huffman_lengths(frequencies: &[u32]) -> Result<Vec<u8>, EncodeError> {
    let mut nodes = Vec::<(Option<usize>, Option<usize>)>::new();
    nodes
        .try_reserve_exact(frequencies.len().saturating_mul(2))
        .map_err(|_| EncodeError::allocation_failed())?;
    nodes.resize(frequencies.len(), (None, None));
    let mut heap = BinaryHeap::new();
    for (symbol, &frequency) in frequencies.iter().enumerate() {
        if frequency != 0 {
            heap.push(Reverse((u64::from(frequency), symbol)));
        }
    }
    if heap.is_empty() {
        heap.push(Reverse((1, 0)));
    }
    if heap.len() == 1 {
        let symbol = heap.peek().expect("one Huffman symbol").0.1;
        let mut lengths = vec![0_u8; frequencies.len()];
        lengths[symbol] = 1;
        return Ok(lengths);
    }
    while heap.len() > 1 {
        let Reverse((left_weight, left)) = heap.pop().expect("left Huffman node");
        let Reverse((right_weight, right)) = heap.pop().expect("right Huffman node");
        let index = nodes.len();
        nodes.push((Some(left), Some(right)));
        heap.push(Reverse((
            left_weight
                .checked_add(right_weight)
                .ok_or_else(EncodeError::output_size_overflow)?,
            index,
        )));
    }
    let root = heap.pop().expect("Huffman root").0.1;
    let mut raw_lengths = vec![0_u16; frequencies.len()];
    let mut stack = vec![(root, 0_u16)];
    let mut maximum = 0_u16;
    while let Some((node, depth)) = stack.pop() {
        if node < frequencies.len() {
            raw_lengths[node] = depth;
            maximum = maximum.max(depth);
        } else {
            let next_depth = depth
                .checked_add(1)
                .ok_or_else(EncodeError::output_size_overflow)?;
            let (left, right) = nodes[node];
            stack.push((right.expect("Huffman right child"), next_depth));
            stack.push((left.expect("Huffman left child"), next_depth));
        }
    }
    if maximum <= 15 {
        return raw_lengths
            .into_iter()
            .map(|length| u8::try_from(length).map_err(|_| EncodeError::output_size_overflow()))
            .collect();
    }

    // Deflate-style overflow repair preserves a complete tree while bounding
    // every code at VP8L's fifteen-bit limit. The least frequent symbols are
    // assigned the repaired longest lengths deterministically.
    let mut length_counts = [0_usize; 16];
    let mut overflow = 0_usize;
    for &length in &raw_lengths {
        if length != 0 {
            let bounded = usize::from(length.min(15));
            length_counts[bounded] += 1;
            overflow += usize::from(length > 15);
        }
    }
    while overflow != 0 {
        let mut length = 14_usize;
        while length != 0 && length_counts[length] == 0 {
            length -= 1;
        }
        if length == 0 || length_counts[15] == 0 {
            return prepare_adaptive_table(frequencies).map(|value| value.0);
        }
        length_counts[length] -= 1;
        length_counts[length + 1] += 2;
        length_counts[15] -= 1;
        overflow = overflow.saturating_sub(2);
    }
    let mut ranked = frequencies
        .iter()
        .enumerate()
        .filter_map(|(symbol, &frequency)| (frequency != 0).then_some((frequency, symbol)))
        .collect::<Vec<_>>();
    ranked.sort_unstable_by(|left, right| left.0.cmp(&right.0).then_with(|| right.1.cmp(&left.1)));
    let mut lengths = vec![0_u8; frequencies.len()];
    let mut ranked_index = 0_usize;
    for length in (1..=15).rev() {
        for _ in 0..length_counts[length] {
            let symbol = ranked
                .get(ranked_index)
                .ok_or_else(EncodeError::output_size_overflow)?
                .1;
            lengths[symbol] = length as u8;
            ranked_index += 1;
        }
    }
    if ranked_index != ranked.len() {
        return Err(EncodeError::output_size_overflow());
    }
    Ok(lengths)
}

fn write_simple_header(
    writer: &mut BitWriter,
    first: u8,
    second: Option<u8>,
) -> Result<(), EncodeError> {
    super::write_bits(writer, 1, 1)?;
    super::write_bits(writer, u32::from(second.is_some()), 1)?;
    let first_is_byte = first > 1;
    super::write_bits(writer, u32::from(first_is_byte), 1)?;
    super::write_bits(writer, u32::from(first), if first_is_byte { 8 } else { 1 })?;
    if let Some(second) = second {
        super::write_bits(writer, u32::from(second), 8)?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "entropy_plan_tests.rs"]
mod tests;
