use crate::{DecodeError, DecodeErrorKind};

/// A least-significant-bit-first reader used by VP8L bitstreams.
#[derive(Clone, Debug)]
pub struct BitReader<'a> {
    data: &'a [u8],
    bit_position: usize,
}

impl<'a> BitReader<'a> {
    #[must_use]
    pub const fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            bit_position: 0,
        }
    }

    /// Creates a reader at an existing bit cursor.  The end position is valid.
    pub fn with_bit_position(data: &'a [u8], bit_position: usize) -> Result<Self, DecodeError> {
        let total_bits = data.len().checked_mul(8).ok_or_else(|| {
            DecodeError::new(
                DecodeErrorKind::InvalidParameter,
                None,
                "input bit length overflow",
            )
        })?;
        if bit_position > total_bits {
            return Err(DecodeError::new(
                DecodeErrorKind::InvalidParameter,
                None,
                "initial bit position is past input",
            ));
        }
        Ok(Self { data, bit_position })
    }

    #[must_use]
    pub const fn bit_position(&self) -> usize {
        self.bit_position
    }

    #[must_use]
    pub fn remaining_bits(&self) -> usize {
        self.data
            .len()
            .saturating_mul(8)
            .saturating_sub(self.bit_position)
    }

    pub fn read_bit(&mut self) -> Result<bool, DecodeError> {
        Ok(self.read_bits(1)? != 0)
    }

    /// Reads up to 32 bits, with the first bit becoming bit 0 of the result.
    /// On failure the cursor is left unchanged.
    pub fn read_bits(&mut self, count: u8) -> Result<u32, DecodeError> {
        if count > 32 {
            return Err(DecodeError::new(
                DecodeErrorKind::InvalidParameter,
                Some(self.bit_position / 8),
                "cannot read more than 32 bits",
            ));
        }
        let count = usize::from(count);
        let end = self.bit_position.checked_add(count).ok_or_else(|| {
            DecodeError::new(
                DecodeErrorKind::UnexpectedEof,
                Some(self.bit_position / 8),
                "bit position overflow",
            )
        })?;
        let total_bits = self.data.len().saturating_mul(8);
        if end > total_bits {
            return Err(DecodeError::new(
                DecodeErrorKind::UnexpectedEof,
                Some(self.bit_position / 8),
                "truncated bitstream",
            ));
        }

        let mut value = 0_u32;
        for output_bit in 0..count {
            let position = self.bit_position + output_bit;
            let bit = (self.data[position / 8] >> (position % 8)) & 1;
            value |= u32::from(bit) << output_bit;
        }
        self.bit_position = end;
        Ok(value)
    }
}

/// A least-significant-bit-first writer.  The final byte is zero-padded.
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

    pub fn write_bits(&mut self, value: u32, count: u8) -> Result<(), DecodeError> {
        if count > 32 {
            return Err(DecodeError::new(
                DecodeErrorKind::InvalidParameter,
                None,
                "cannot write more than 32 bits",
            ));
        }
        let count = usize::from(count);
        let new_len = self.bit_len.checked_add(count).ok_or_else(|| {
            DecodeError::new(
                DecodeErrorKind::AllocationFailed,
                None,
                "bit writer length overflow",
            )
        })?;
        let bytes = new_len.div_ceil(8);
        if bytes > self.data.len() {
            self.data
                .try_reserve(bytes - self.data.len())
                .map_err(|_| {
                    DecodeError::new(
                        DecodeErrorKind::AllocationFailed,
                        None,
                        "bit writer allocation failed",
                    )
                })?;
            self.data.resize(bytes, 0);
        }
        for input_bit in 0..count {
            if ((value >> input_bit) & 1) != 0 {
                let position = self.bit_len + input_bit;
                self.data[position / 8] |= 1 << (position % 8);
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

#[cfg(test)]
mod tests {
    use super::*;

    fn slow_read(data: &[u8], start: usize, count: u8) -> Option<u32> {
        let end = start.checked_add(usize::from(count))?;
        if end > data.len().checked_mul(8)? {
            return None;
        }
        let mut value = 0;
        for bit_index in 0..usize::from(count) {
            value |= u32::from((data[(start + bit_index) / 8] >> ((start + bit_index) % 8)) & 1)
                << bit_index;
        }
        Some(value)
    }

    #[test]
    fn reads_all_offsets_and_widths_like_slow_model() {
        for data in [&[][..], &[0x00][..], &[0xff, 0x00, 0xaa, 0x55, 0xff][..]] {
            for start in 0..=data.len() * 8 {
                for count in 0..=32 {
                    let expected = slow_read(data, start, count);
                    let mut reader = BitReader::with_bit_position(data, start).unwrap();
                    assert_eq!(reader.bit_position(), start);
                    assert_eq!(reader.remaining_bits(), data.len() * 8 - start);
                    match expected {
                        Some(expected) => {
                            assert_eq!(reader.read_bits(count).unwrap(), expected);
                            assert_eq!(reader.bit_position(), start + usize::from(count));
                            assert_eq!(
                                reader.remaining_bits(),
                                data.len() * 8 - start - usize::from(count)
                            );
                        }
                        None => {
                            assert!(reader.read_bits(count).is_err());
                            assert_eq!(reader.bit_position(), start);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn read_bit_reports_each_boolean_value_and_advances_once() {
        let mut reader = BitReader::new(&[0b0000_0010]);
        assert_eq!(reader.read_bit(), Ok(false));
        assert_eq!(reader.bit_position(), 1);
        assert_eq!(reader.read_bit(), Ok(true));
        assert_eq!(reader.bit_position(), 2);
        assert_eq!(reader.remaining_bits(), 6);
    }

    #[test]
    fn failed_read_does_not_advance_cursor() {
        let mut reader = BitReader::new(&[0b0000_0011]);
        assert_eq!(reader.read_bits(7), Ok(3));
        let cursor = reader.bit_position();
        assert_eq!(
            reader.read_bits(2).unwrap_err().kind(),
            DecodeErrorKind::UnexpectedEof
        );
        assert_eq!(reader.bit_position(), cursor);
        assert_eq!(reader.read_bits(1), Ok(0));
    }

    #[test]
    fn zero_and_thirty_two_bit_reads_are_valid() {
        let mut reader = BitReader::new(&[0x78, 0x56, 0x34, 0x12]);
        assert_eq!(reader.read_bits(0), Ok(0));
        assert_eq!(reader.read_bits(32), Ok(0x1234_5678));
    }

    #[test]
    fn writer_reader_round_trip_across_boundaries() {
        let values = [
            (1_u32, 1_u8),
            (0b101, 3),
            (0x5a, 8),
            (0x00ab_cdef, 20),
            (0xffff_ffff, 32),
        ];
        let mut writer = BitWriter::new();
        let mut expected_bits = 0;
        for &(value, width) in &values {
            writer.write_bits(value, width).unwrap();
            expected_bits += usize::from(width);
            assert_eq!(writer.bit_len(), expected_bits);
        }
        let encoded = writer.as_bytes().to_vec();
        let mut reader = BitReader::new(writer.as_bytes());
        for &(value, width) in &values {
            let mask = if width == 32 {
                u32::MAX
            } else {
                (1_u32 << width) - 1
            };
            assert_eq!(reader.read_bits(width), Ok(value & mask));
        }
        assert_eq!(writer.into_bytes(), encoded);
    }

    #[test]
    fn invalid_width_is_rejected() {
        let mut reader = BitReader::with_bit_position(&[0; 3], 17).unwrap();
        let invalid_width = reader.read_bits(33).unwrap_err();
        assert_eq!(invalid_width.kind(), DecodeErrorKind::InvalidParameter);
        assert_eq!(invalid_width.offset(), Some(2));
        let truncated = reader.read_bits(8).unwrap_err();
        assert_eq!(truncated.kind(), DecodeErrorKind::UnexpectedEof);
        assert_eq!(truncated.offset(), Some(2));
        let mut writer = BitWriter::new();
        assert_eq!(
            writer.write_bits(0, 33).unwrap_err().kind(),
            DecodeErrorKind::InvalidParameter
        );
    }
}
