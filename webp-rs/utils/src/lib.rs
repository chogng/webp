#![forbid(unsafe_code)]
//! Small format-neutral infrastructure shared across WebP crates.

use core::fmt;

/// Failure while growing or addressing a least-significant-bit-first stream.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BitWriteError {
    /// A single write requested more than 32 bits.
    InvalidWidth,
    /// The output buffer could not be grown or its length overflowed.
    AllocationFailed,
}

impl fmt::Display for BitWriteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidWidth => formatter.write_str("cannot write more than 32 bits"),
            Self::AllocationFailed => formatter.write_str("bit writer allocation failed"),
        }
    }
}

impl std::error::Error for BitWriteError {}

/// A least-significant-bit-first writer. The final byte is zero-padded.
#[derive(Clone, Debug, Default)]
pub struct BitWriter {
    data: Vec<u8>,
    bit_len: usize,
}

impl BitWriter {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            data: Vec::new(),
            bit_len: 0,
        }
    }

    #[must_use]
    pub const fn bit_len(&self) -> usize {
        self.bit_len
    }

    /// Takes ownership of an already packed LSB-first prefix.
    ///
    /// Returns `None` unless `data` has exactly the bytes required by
    /// `bit_len` and every unused high bit in the final byte is zero.
    #[must_use]
    pub fn from_bytes(data: Vec<u8>, bit_len: usize) -> Option<Self> {
        if data.len() != bit_len.div_ceil(8) {
            return None;
        }
        let used = bit_len % 8;
        if used != 0
            && data
                .last()
                .is_some_and(|byte| byte & !((1_u8 << used) - 1) != 0)
        {
            return None;
        }
        Some(Self { data, bit_len })
    }

    /// Appends the low `count` bits of `value` in least-significant-bit order.
    ///
    /// # Errors
    ///
    /// Returns [`BitWriteError::InvalidWidth`] when `count` exceeds 32, or
    /// [`BitWriteError::AllocationFailed`] when the output cannot be grown.
    pub fn write_bits(&mut self, value: u32, count: u8) -> Result<(), BitWriteError> {
        if count > 32 {
            return Err(BitWriteError::InvalidWidth);
        }
        let count = usize::from(count);
        let new_len = self
            .bit_len
            .checked_add(count)
            .ok_or(BitWriteError::AllocationFailed)?;
        let bytes = new_len.div_ceil(8);
        if bytes > self.data.len() {
            self.data
                .try_reserve(bytes - self.data.len())
                .map_err(|_| BitWriteError::AllocationFailed)?;
            self.data.resize(bytes, 0);
        }
        if count != 0 {
            let byte_offset = self.bit_len / 8;
            let bit_offset = self.bit_len % 8;
            let mask = (1_u64 << count) - 1;
            let pending = (u64::from(value) & mask) << bit_offset;
            let pending_bytes = (bit_offset + count).div_ceil(8);
            for index in 0..pending_bytes {
                self.data[byte_offset + index] |= (pending >> (index * 8)) as u8;
            }
        }
        self.bit_len = new_len;
        Ok(())
    }

    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.data
    }

    #[must_use]
    pub fn into_bytes(self) -> Vec<u8> {
        self.data
    }
}

/// Decodes a three-byte little-endian unsigned integer.
#[must_use]
pub const fn read_u24_le(bytes: [u8; 3]) -> u32 {
    u32::from_le_bytes([bytes[0], bytes[1], bytes[2], 0])
}

/// Encodes the low 24 bits of an unsigned integer in little-endian order.
#[must_use]
pub const fn write_u24_le(value: u32) -> [u8; 3] {
    let bytes = value.to_le_bytes();
    [bytes[0], bytes[1], bytes[2]]
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
