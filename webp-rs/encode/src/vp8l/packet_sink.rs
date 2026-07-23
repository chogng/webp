//! Shared packed VP8L token serialization for every entropy profile.

use super::{
    BitWriter, CHANNEL_ALPHABET_SIZE, DISTANCE_ALPHABET_SIZE, EncodeError, EncodingTable,
    EntropyTables, EntropyToken, FIRST_CACHE_SYMBOL, vp8l_prefix,
};

/// One complete token in the LSB-first order used on the VP8L wire.
#[derive(Clone, Copy)]
struct TokenPacket {
    bits: u64,
    width: u8,
}

impl TokenPacket {
    const fn new() -> Self {
        Self { bits: 0, width: 0 }
    }

    fn push_wire(&mut self, value: u32, width: u8) -> Result<(), EncodeError> {
        if width == 0 {
            return Ok(());
        }
        if width > u32::BITS as u8 {
            return Err(EncodeError::output_size_overflow());
        }
        let next_width = self
            .width
            .checked_add(width)
            .ok_or_else(EncodeError::output_size_overflow)?;
        if next_width > u64::BITS as u8 {
            return Err(EncodeError::output_size_overflow());
        }
        let mask = if width == u32::BITS as u8 {
            u64::from(u32::MAX)
        } else {
            (1_u64 << width) - 1
        };
        self.bits |= (u64::from(value) & mask) << self.width;
        self.width = next_width;
        Ok(())
    }

    fn push_symbol(&mut self, table: &EncodingTable, symbol: usize) -> Result<(), EncodeError> {
        let (code, width) = table
            .codes
            .get(symbol)
            .copied()
            .ok_or_else(EncodeError::output_size_overflow)?;
        if width == 0 {
            return Ok(());
        }
        if width > u32::BITS as u8 {
            return Err(EncodeError::output_size_overflow());
        }
        let wire = code.reverse_bits() >> (u32::BITS - u32::from(width));
        self.push_wire(wire, width)
    }
}

fn packet_for_token(
    token: EntropyToken,
    tables: &EntropyTables,
) -> Result<TokenPacket, EncodeError> {
    let mut packet = TokenPacket::new();
    match token {
        EntropyToken::Cache(index) => {
            let symbol = FIRST_CACHE_SYMBOL
                .checked_add(index)
                .ok_or_else(EncodeError::output_size_overflow)?;
            packet.push_symbol(&tables.green, symbol)?;
        }
        EntropyToken::Literal(rgba) => {
            packet.push_symbol(&tables.green, usize::from(rgba[1]))?;
            packet.push_symbol(&tables.red, usize::from(rgba[0]))?;
            packet.push_symbol(&tables.blue, usize::from(rgba[2]))?;
            packet.push_symbol(&tables.alpha, usize::from(rgba[3]))?;
        }
        EntropyToken::Copy {
            length,
            distance_code,
        } => {
            let (length_prefix, length_extra) = vp8l_prefix(length, 24)?;
            packet.push_symbol(&tables.green, CHANNEL_ALPHABET_SIZE + length_prefix)?;
            packet.push_wire(length_extra.0, length_extra.1)?;
            let (distance_prefix, distance_extra) =
                vp8l_prefix(distance_code, DISTANCE_ALPHABET_SIZE)?;
            packet.push_symbol(&tables.distance, distance_prefix)?;
            packet.push_wire(distance_extra.0, distance_extra.1)?;
        }
    }
    Ok(packet)
}

fn packet_reserve_bytes(token_bits: usize) -> Result<usize, EncodeError> {
    token_bits
        .checked_add(7)
        .map(|bits| bits / 8)
        .and_then(|bytes| bytes.checked_add(size_of::<u32>()))
        .ok_or_else(EncodeError::output_size_overflow)
}

/// A capacity-bounded sink that owns the prefix and flushes low 32-bit words.
pub(super) struct PackedTokenWriter {
    data: Vec<u8>,
    accumulator: u64,
    used: u8,
    bit_len: usize,
}

impl PackedTokenWriter {
    pub(super) fn from_prefix(prefix: BitWriter, token_bits: usize) -> Result<Self, EncodeError> {
        let bit_len = prefix.bit_len();
        Self::from_parts(
            prefix.into_bytes(),
            bit_len,
            packet_reserve_bytes(token_bits)?,
        )
    }

    fn from_parts(
        mut data: Vec<u8>,
        bit_len: usize,
        reserve_bytes: usize,
    ) -> Result<Self, EncodeError> {
        let full_bytes = bit_len / 8;
        let used = (bit_len % 8) as u8;
        let prefix_bytes = full_bytes
            .checked_add(usize::from(used != 0))
            .ok_or_else(EncodeError::output_size_overflow)?;
        if data.len() != prefix_bytes {
            return Err(EncodeError::output_size_overflow());
        }
        let accumulator = if used == 0 {
            0
        } else {
            u64::from(data[full_bytes] & ((1_u8 << used) - 1))
        };
        data.truncate(full_bytes);
        data.try_reserve(reserve_bytes)
            .map_err(|_| EncodeError::allocation_failed())?;
        Ok(Self {
            data,
            accumulator,
            used,
            bit_len,
        })
    }

    pub(super) fn write_token(
        &mut self,
        token: EntropyToken,
        tables: &EntropyTables,
    ) -> Result<(), EncodeError> {
        self.append(packet_for_token(token, tables)?)
    }

    fn append(&mut self, packet: TokenPacket) -> Result<(), EncodeError> {
        if packet.width == 0 {
            return Ok(());
        }
        if packet.width > u64::BITS as u8 || self.used >= u32::BITS as u8 {
            return Err(EncodeError::output_size_overflow());
        }
        let mask = if packet.width == u64::BITS as u8 {
            u64::MAX
        } else {
            (1_u64 << packet.width) - 1
        };
        let mut pending =
            u128::from(self.accumulator) | (u128::from(packet.bits & mask) << self.used);
        let mut used = self
            .used
            .checked_add(packet.width)
            .ok_or_else(EncodeError::output_size_overflow)?;
        self.bit_len = self
            .bit_len
            .checked_add(usize::from(packet.width))
            .ok_or_else(EncodeError::output_size_overflow)?;
        while used >= u32::BITS as u8 {
            let end = self
                .data
                .len()
                .checked_add(size_of::<u32>())
                .ok_or_else(EncodeError::output_size_overflow)?;
            if end > self.data.capacity() {
                return Err(EncodeError::allocation_failed());
            }
            self.data.extend_from_slice(&(pending as u32).to_le_bytes());
            pending >>= u32::BITS;
            used -= u32::BITS as u8;
        }
        self.accumulator = pending as u64;
        self.used = used;
        Ok(())
    }

    pub(super) const fn bit_len(&self) -> usize {
        self.bit_len
    }

    pub(super) fn into_prefix(self) -> Result<BitWriter, EncodeError> {
        let bit_len = self.bit_len;
        BitWriter::from_bytes(self.finish()?, bit_len).ok_or_else(EncodeError::output_size_overflow)
    }

    pub(super) fn finish(mut self) -> Result<Vec<u8>, EncodeError> {
        let remaining = usize::from(self.used).div_ceil(8);
        let end = self
            .data
            .len()
            .checked_add(remaining)
            .ok_or_else(EncodeError::output_size_overflow)?;
        if end > self.data.capacity() {
            return Err(EncodeError::allocation_failed());
        }
        self.data
            .extend_from_slice(&self.accumulator.to_le_bytes()[..remaining]);
        Ok(self.data)
    }
}

#[cfg(test)]
#[path = "packet_sink_tests.rs"]
mod tests;
