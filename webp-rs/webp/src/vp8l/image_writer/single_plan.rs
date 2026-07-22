//! Exact same-profile single-group Huffman preparation and size accounting.

use super::{
    BitWriter, CHANNEL_ALPHABET_SIZE, DISTANCE_ALPHABET_SIZE, EncodeError, EntropyFrequencies,
    EntropyTables, prepare_adaptive_table, vp8l_prefix, write_bits, write_normal_table,
};

const FAST_PREFIX_BITS: usize = 44;
const NORMAL_TABLE_FIXED_BITS: usize = 63;

pub(crate) struct SinglePlan {
    green_lengths: Vec<u8>,
    red_lengths: Vec<u8>,
    blue_lengths: Vec<u8>,
    alpha_lengths: Vec<u8>,
    distance_lengths: Vec<u8>,
    tables: EntropyTables,
    payload_bits: usize,
    riff_bytes: usize,
}

impl SinglePlan {
    pub(crate) fn build(frequencies: &EntropyFrequencies) -> Result<Self, EncodeError> {
        let (green_lengths, green) =
            prepare_adaptive_table(&frequencies.green[..frequencies.green_len])?;
        let (red_lengths, red) = prepare_adaptive_table(&frequencies.red)?;
        let (blue_lengths, blue) = prepare_adaptive_table(&frequencies.blue)?;
        let (alpha_lengths, alpha) = prepare_adaptive_table(&frequencies.alpha)?;
        let (distance_lengths, distance) = prepare_adaptive_table(&frequencies.distance)?;
        let tables = EntropyTables {
            green,
            red,
            blue,
            alpha,
            distance,
        };

        let mut payload_bits = FAST_PREFIX_BITS
            .checked_add(2)
            .ok_or_else(EncodeError::output_size_overflow)?;
        for lengths in [
            &green_lengths,
            &red_lengths,
            &blue_lengths,
            &alpha_lengths,
            &distance_lengths,
        ] {
            payload_bits = payload_bits
                .checked_add(table_header_bits(lengths.len())?)
                .ok_or_else(EncodeError::output_size_overflow)?;
        }
        for (frequency, (_, width)) in frequencies.green[..frequencies.green_len]
            .iter()
            .zip(&tables.green.codes)
        {
            payload_bits = add_weighted_width(payload_bits, *frequency, *width)?;
        }
        for (frequencies, table) in [
            (&frequencies.red[..], &tables.red),
            (&frequencies.blue[..], &tables.blue),
            (&frequencies.alpha[..], &tables.alpha),
            (&frequencies.distance[..], &tables.distance),
        ] {
            for (frequency, (_, width)) in frequencies.iter().zip(&table.codes) {
                payload_bits = add_weighted_width(payload_bits, *frequency, *width)?;
            }
        }

        let mut green_copy_count = 0_u64;
        for prefix in 0..24 {
            let frequency = frequencies.green[CHANNEL_ALPHABET_SIZE + prefix];
            green_copy_count = green_copy_count
                .checked_add(u64::from(frequency))
                .ok_or_else(EncodeError::output_size_overflow)?;
            let extra_bits = if prefix < 4 { 0 } else { (prefix - 2) >> 1 };
            payload_bits = add_weighted_width(
                payload_bits,
                frequency,
                u8::try_from(extra_bits).map_err(|_| EncodeError::output_size_overflow())?,
            )?;
        }
        let distance_copy_count =
            frequencies
                .distance
                .iter()
                .try_fold(0_u64, |count, &frequency| {
                    count
                        .checked_add(u64::from(frequency))
                        .ok_or_else(EncodeError::output_size_overflow)
                })?;
        if green_copy_count != distance_copy_count {
            return Err(EncodeError::output_size_overflow());
        }
        let (_, (_, distance_extra_bits)) = vp8l_prefix(121, DISTANCE_ALPHABET_SIZE)?;
        let distance_copy_count = usize::try_from(distance_copy_count)
            .map_err(|_| EncodeError::output_size_overflow())?;
        payload_bits = distance_copy_count
            .checked_mul(usize::from(distance_extra_bits))
            .and_then(|bits| payload_bits.checked_add(bits))
            .ok_or_else(EncodeError::output_size_overflow)?;

        let payload_bytes = payload_bits
            .checked_add(7)
            .map(|bits| bits / 8)
            .ok_or_else(EncodeError::output_size_overflow)?;
        let padded_payload_bytes = payload_bytes
            .checked_add(payload_bytes & 1)
            .ok_or_else(EncodeError::output_size_overflow)?;
        let riff_size = 12_usize
            .checked_add(padded_payload_bytes)
            .ok_or_else(EncodeError::output_size_overflow)?;
        u32::try_from(riff_size).map_err(|_| EncodeError::output_size_overflow())?;
        let riff_bytes = riff_size
            .checked_add(8)
            .ok_or_else(EncodeError::output_size_overflow)?;

        Ok(Self {
            green_lengths,
            red_lengths,
            blue_lengths,
            alpha_lengths,
            distance_lengths,
            tables,
            payload_bits,
            riff_bytes,
        })
    }

    pub(crate) fn write_main_prefix(&self, bits: &mut BitWriter) -> Result<(), EncodeError> {
        write_bits(bits, 0, 1)?; // No color cache.
        write_bits(bits, 0, 1)?; // One entropy-code group.
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

    pub(crate) const fn payload_bits(&self) -> usize {
        self.payload_bits
    }

    #[cfg(test)]
    pub(crate) const fn payload_bytes(&self) -> usize {
        self.payload_bits.div_ceil(8)
    }

    pub(crate) const fn riff_bytes(&self) -> usize {
        self.riff_bytes
    }
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
#[path = "single_plan_tests.rs"]
mod tests;
