//! VP8 boolean arithmetic decoding over already-bounded partitions.

use webp_core::DecodeError;
use webp_core::DecodeErrorKind;
use webp_core::DecodeLimits;
use webp_core::WorkBudget;

/// Failure while building a VP8 boolean-coded partition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BoolEncodeError {
    AllocationFailed,
    InvalidLiteralWidth,
}

/// VP8's most-significant-bit-first boolean arithmetic encoder.
///
/// This is the writer counterpart to [`BoolDecoder`]. It keeps its arithmetic
/// interval in the VP8 `range - 1` form and delays carry propagation exactly
/// as the format's byte writer requires.
#[derive(Debug)]
pub struct BoolEncoder {
    range: i32,
    value: i32,
    run: usize,
    pending_bits: i32,
    bytes: Vec<u8>,
}

impl Default for BoolEncoder {
    fn default() -> Self {
        Self::new()
    }
}

impl BoolEncoder {
    #[must_use]
    pub fn new() -> Self {
        Self {
            range: 254,
            value: 0,
            run: 0,
            pending_bits: -8,
            bytes: Vec::new(),
        }
    }

    pub fn write_bool(&mut self, bit: bool, probability: u8) -> Result<(), BoolEncodeError> {
        let split = (self.range * i32::from(probability)) >> 8;
        if bit {
            self.value += split + 1;
            self.range -= split + 1;
        } else {
            self.range = split;
        }
        if self.range < 127 {
            let shift = 7 - (self.range + 1).ilog2() as i32;
            self.range = ((self.range + 1) << shift) - 1;
            self.value <<= shift;
            self.pending_bits += shift;
            if self.pending_bits > 0 {
                self.flush()?;
            }
        }
        Ok(())
    }

    pub fn write_literal(&mut self, value: u32, count: u8) -> Result<(), BoolEncodeError> {
        if count > 32 {
            return Err(BoolEncodeError::InvalidLiteralWidth);
        }
        for shift in (0..count).rev() {
            self.write_bool(((value >> shift) & 1) != 0, 128)?;
        }
        Ok(())
    }

    pub fn write_signed_literal(&mut self, value: i32, count: u8) -> Result<(), BoolEncodeError> {
        self.write_literal(value.unsigned_abs(), count)?;
        self.write_bool(value.is_negative(), 128)
    }

    pub fn finish(mut self) -> Result<Vec<u8>, BoolEncodeError> {
        self.write_literal(0, (9 - self.pending_bits) as u8)?;
        self.pending_bits = 0;
        self.flush()?;
        Ok(self.bytes)
    }

    fn flush(&mut self) -> Result<(), BoolEncodeError> {
        let shift = 8 + self.pending_bits;
        let bits = self.value >> shift;
        self.value -= bits << shift;
        self.pending_bits -= 8;
        if bits & 0xff == 0xff {
            self.run = self
                .run
                .checked_add(1)
                .ok_or(BoolEncodeError::AllocationFailed)?;
            return Ok(());
        }
        let extra = self
            .run
            .checked_add(1)
            .ok_or(BoolEncodeError::AllocationFailed)?;
        self.bytes
            .try_reserve(extra)
            .map_err(|_| BoolEncodeError::AllocationFailed)?;
        if bits & 0x100 != 0
            && let Some(previous) = self.bytes.last_mut()
        {
            *previous = previous.wrapping_add(1);
        }
        let delayed = if bits & 0x100 != 0 { 0 } else { 0xff };
        self.bytes.extend(std::iter::repeat_n(delayed, self.run));
        self.run = 0;
        self.bytes.push((bits & 0xff) as u8);
        Ok(())
    }
}

/// VP8's most-significant-bit-first arithmetic boolean decoder.
///
/// The decoder owns a deterministic work budget: every decoded boolean value
/// consumes one unit. It never fabricates zero-padding beyond the supplied
/// partition; callers receive [`DecodeErrorKind::UnexpectedEof`] instead.
#[derive(Clone, Debug)]
pub struct BoolDecoder<'a> {
    data: &'a [u8],
    byte_position: usize,
    value: u64,
    /// VP8 stores the active interval as `range - 1`.
    range: u32,
    /// Number of cached low bits usable as the comparison position.
    bits: i32,
    work: WorkBudget,
}

impl<'a> BoolDecoder<'a> {
    /// Creates a decoder over one already-bounded VP8 partition.
    pub fn new(data: &'a [u8], limits: &DecodeLimits) -> Result<Self, DecodeError> {
        limits.check_input_len(data.len())?;
        Ok(Self {
            data,
            byte_position: 0,
            value: 0,
            range: 254,
            bits: -8,
            work: limits.work_budget(),
        })
    }

    /// Decodes one boolean value with the supplied VP8 probability.
    pub fn read_bool(&mut self, probability: u8) -> Result<bool, DecodeError> {
        self.work.consume(1)?;
        if self.bits < 0 {
            self.load_byte()?;
        }

        let split = (self.range * u32::from(probability)) >> 8;
        let value = (self.value >> self.bits) as u32;
        let bit = value > split;
        if bit {
            self.range -= split;
            self.value -= u64::from(split + 1) << self.bits;
        } else {
            self.range = split + 1;
        }

        let shift = 7 - self.range.ilog2() as i32;
        self.range <<= shift;
        self.bits -= shift;
        self.range -= 1;
        Ok(bit)
    }

    /// Reads a fixed-width, most-significant-bit-first VP8 literal.
    pub fn read_literal(&mut self, count: u8) -> Result<u32, DecodeError> {
        if count > 32 {
            return Err(DecodeError::at(
                DecodeErrorKind::InvalidParameter,
                self.byte_position,
                "VP8 literal width exceeds 32 bits",
            ));
        }
        let mut value = 0_u32;
        for _ in 0..count {
            value = (value << 1) | u32::from(self.read_bool(128)?);
        }
        Ok(value)
    }

    /// Reads a VP8 sign-magnitude value: magnitude first, then its sign bit.
    pub fn read_signed_literal(&mut self, count: u8) -> Result<i32, DecodeError> {
        let raw_magnitude = self.read_literal(count)?;
        let magnitude = i32::try_from(raw_magnitude).map_err(|_| {
            DecodeError::at(
                DecodeErrorKind::InvalidBitstream,
                self.byte_position,
                "VP8 signed literal does not fit i32",
            )
        })?;
        if self.read_bool(128)? {
            Ok(-magnitude)
        } else {
            Ok(magnitude)
        }
    }

    /// Number of input bytes consumed from this partition.
    #[must_use]
    pub const fn bytes_consumed(&self) -> usize {
        self.byte_position
    }

    /// Remaining deterministic decoder work units.
    #[must_use]
    pub const fn remaining_work(&self) -> u64 {
        self.work.remaining()
    }

    fn load_byte(&mut self) -> Result<(), DecodeError> {
        let byte = *self.data.get(self.byte_position).ok_or_else(|| {
            DecodeError::at(
                DecodeErrorKind::UnexpectedEof,
                self.byte_position,
                "truncated VP8 boolean-coded partition",
            )
        })?;
        self.byte_position += 1;
        self.value = u64::from(byte) | (self.value << 8);
        self.bits += 8;
        Ok(())
    }
}

#[cfg(test)]
#[path = "bitstream_tests.rs"]
mod tests;
