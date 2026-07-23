//! Candidate-independent Huffman preparation and exact VP8L bit accounting.

use super::token_stream::{EntropyFrequencies, MAX_ENCODER_COLOR_CACHE_BITS, TokenStatistics};
use super::{
    BitWriter, CHANNEL_ALPHABET_SIZE, DISTANCE_ALPHABET_SIZE, EncodeError, EntropyTables,
    prepare_adaptive_table, vp8l_prefix, write_bits, write_normal_table,
};

const NORMAL_TABLE_FIXED_BITS: usize = 63;

pub(crate) struct EntropyPlan {
    green_lengths: Vec<u8>,
    red_lengths: Vec<u8>,
    blue_lengths: Vec<u8>,
    alpha_lengths: Vec<u8>,
    distance_lengths: Vec<u8>,
    tables: EntropyTables,
    table_bits: usize,
    token_bits: usize,
    secondary_lookups: usize,
}

impl EntropyPlan {
    pub(crate) fn build_for_stream(statistics: &TokenStatistics) -> Result<Self, EncodeError> {
        let plan = Self::build(statistics.frequencies())?;
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
        let (green_lengths, green) = prepare_adaptive_table(frequencies.green())?;
        let (red_lengths, red) = prepare_adaptive_table(frequencies.red())?;
        let (blue_lengths, blue) = prepare_adaptive_table(frequencies.blue())?;
        let (alpha_lengths, alpha) = prepare_adaptive_table(frequencies.alpha())?;
        let (distance_lengths, distance) = prepare_adaptive_table(frequencies.distance())?;
        let tables = EntropyTables {
            green,
            red,
            blue,
            alpha,
            distance,
        };

        let mut table_bits = 0_usize;
        for lengths in [
            &green_lengths,
            &red_lengths,
            &blue_lengths,
            &alpha_lengths,
            &distance_lengths,
        ] {
            table_bits = table_bits
                .checked_add(table_header_bits(lengths.len())?)
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
        let distance_copy_count = frequencies
            .distance()
            .iter()
            .try_fold(0_u64, |total, &frequency| {
                total.checked_add(u64::from(frequency))
            })
            .ok_or_else(EncodeError::output_size_overflow)?;
        if green_copy_count != distance_copy_count {
            return Err(EncodeError::output_size_overflow());
        }
        let (_, (_, distance_extra_bits)) = vp8l_prefix(121, DISTANCE_ALPHABET_SIZE)?;
        let distance_copy_count = usize::try_from(distance_copy_count)
            .map_err(|_| EncodeError::output_size_overflow())?;
        token_bits = distance_copy_count
            .checked_mul(usize::from(distance_extra_bits))
            .and_then(|bits| token_bits.checked_add(bits))
            .ok_or_else(EncodeError::output_size_overflow)?;

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
        for lengths in [
            &self.green_lengths,
            &self.red_lengths,
            &self.blue_lengths,
            &self.alpha_lengths,
            &self.distance_lengths,
        ] {
            write_normal_table(bits, lengths)?;
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

fn table_header_bits(alphabet_len: usize) -> Result<usize, EncodeError> {
    alphabet_len
        .checked_mul(4)
        .and_then(|bits| bits.checked_add(NORMAL_TABLE_FIXED_BITS))
        .ok_or_else(EncodeError::output_size_overflow)
}

fn add_weighted_width(total: usize, frequency: u32, width: u8) -> Result<usize, EncodeError> {
    usize::try_from(frequency)
        .ok()
        .and_then(|frequency| frequency.checked_mul(usize::from(width)))
        .and_then(|bits| total.checked_add(bits))
        .ok_or_else(EncodeError::output_size_overflow)
}

#[cfg(test)]
#[path = "entropy_plan_tests.rs"]
mod tests;
