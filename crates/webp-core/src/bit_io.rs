use crate::{DecodeError, DecodeErrorKind};

/// A least-significant-bit-first reader used by VP8L bitstreams.
#[derive(Clone, Debug)]
pub struct BitReader<'a> {
    data: &'a [u8],
    bit_position: usize,
    bit_len: usize,
    window: u64,
    window_byte_position: usize,
    window_bit_offset: u8,
    window_valid: bool,
}

impl<'a> BitReader<'a> {
    #[must_use]
    pub const fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            bit_position: 0,
            bit_len: data.len().saturating_mul(8),
            window: 0,
            window_byte_position: 0,
            window_bit_offset: 0,
            window_valid: false,
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
        Ok(Self {
            data,
            bit_position,
            bit_len: total_bits,
            window: 0,
            window_byte_position: bit_position / 8,
            window_bit_offset: (bit_position % 8) as u8,
            window_valid: false,
        })
    }

    #[must_use]
    pub const fn bit_position(&self) -> usize {
        self.bit_position
    }

    /// Borrows this reader as a shift-register bit buffer for entropy loops.
    ///
    /// The strict `BitReader` API remains unchanged; callers opt into this
    /// representation only while a dense sequence of small reads is active.
    /// Dropping the adapter synchronizes the consumed position back here.
    #[inline]
    pub fn shifted(&mut self) -> ShiftedBitReader<'_, 'a> {
        let bit_position = self.bit_position;
        let byte_position = bit_position / 8;
        let bit_offset = bit_position % 8;
        let (buffer, nbits, next_byte_position) = if bit_offset != 0 {
            match self.data.get(byte_position) {
                Some(&byte) => (
                    u64::from(byte) >> bit_offset,
                    (8 - bit_offset) as u8,
                    byte_position + 1,
                ),
                None => (0, 0, byte_position),
            }
        } else {
            (0, 0, byte_position)
        };
        ShiftedBitReader {
            reader: self,
            buffer,
            nbits,
            next_byte_position,
            bit_position,
        }
    }

    #[must_use]
    #[inline]
    pub fn remaining_bits(&self) -> usize {
        // Every constructor validates the cursor and the mutating operations
        // advance it only after their bounds check succeeds.
        self.bit_len - self.bit_position
    }

    #[inline]
    pub fn read_bit(&mut self) -> Result<bool, DecodeError> {
        Ok(self.read_bits(1)? != 0)
    }

    /// Views up to 32 upcoming bits without advancing the cursor.
    ///
    /// As with [`Self::read_bits`], the first bit becomes bit 0 of the result.
    #[inline]
    pub fn peek_bits(&self, count: u8) -> Result<u32, DecodeError> {
        self.bits_at(self.bit_position, count)
    }

    /// Advances over upcoming bits without returning their value.
    ///
    /// On failure the cursor is left unchanged.
    #[inline]
    pub fn skip_bits(&mut self, count: u8) -> Result<(), DecodeError> {
        let end = self.end_position(self.bit_position, count)?;
        self.set_position(end);
        Ok(())
    }

    /// Moves the cursor backwards by up to 32 already-consumed bits.
    ///
    /// On failure the cursor is left unchanged. This is useful for table
    /// decoders that speculatively read a fixed-width prefix and retain only
    /// the bits belonging to a shorter matching code.
    #[inline]
    pub fn rewind_bits(&mut self, count: u8) -> Result<(), DecodeError> {
        if count > 32 {
            return Err(DecodeError::new(
                DecodeErrorKind::InvalidParameter,
                Some(self.bit_position / 8),
                "cannot rewind more than 32 bits",
            ));
        }
        let position = self
            .bit_position
            .checked_sub(usize::from(count))
            .ok_or_else(|| {
                DecodeError::new(
                    DecodeErrorKind::InvalidParameter,
                    Some(self.bit_position / 8),
                    "cannot rewind before the start of input",
                )
            })?;
        self.set_position(position);
        Ok(())
    }

    /// Reads up to 32 bits, with the first bit becoming bit 0 of the result.
    /// On failure the cursor is left unchanged.
    #[inline]
    pub fn read_bits(&mut self, count: u8) -> Result<u32, DecodeError> {
        let end = self.end_position(self.bit_position, count)?;
        if count == 0 {
            return Ok(0);
        }

        self.prepare_window();
        let count = usize::from(count);
        let mask = if count == 32 {
            u64::from(u32::MAX)
        } else {
            (1_u64 << count) - 1
        };
        let value = ((self.window >> self.window_bit_offset) & mask) as u32;
        self.bit_position += count;
        self.window_bit_offset += count as u8;
        debug_assert_eq!(self.bit_position, end);
        Ok(value)
    }

    #[inline]
    fn prepare_window(&mut self) {
        if !self.window_valid || self.window_bit_offset >= 32 {
            self.reload_window();
        }
        debug_assert!(self.window_bit_offset < 32);
    }

    #[inline]
    fn reload_window(&mut self) {
        let byte_position = self.bit_position / 8;
        let remaining = &self.data[byte_position..];
        self.window = if let Some(bytes) = remaining.get(..8) {
            u64::from_le_bytes(bytes.try_into().expect("eight-byte window"))
        } else {
            let mut window = 0_u64;
            for (index, &byte) in remaining.iter().enumerate() {
                window |= u64::from(byte) << (index * 8);
            }
            window
        };
        self.window_byte_position = byte_position;
        self.window_bit_offset = (self.bit_position % 8) as u8;
        self.window_valid = true;
    }

    #[inline]
    fn set_position(&mut self, position: usize) {
        self.bit_position = position;
        if !self.window_valid {
            return;
        }
        let window_start = self.window_byte_position * 8;
        let Some(offset) = position.checked_sub(window_start) else {
            self.window_valid = false;
            return;
        };
        if offset <= 64 {
            self.window_bit_offset = offset as u8;
        } else {
            self.window_valid = false;
        }
    }

    #[inline]
    fn bits_at(&self, position: usize, count: u8) -> Result<u32, DecodeError> {
        let end = self.end_position(position, count)?;
        let count = usize::from(count);
        if count == 0 {
            return Ok(0);
        }

        // A requested 32-bit field can start up to seven bits into a byte, so
        // it spans at most five bytes. Assemble that small LSB-first window
        // once instead of visiting every requested bit individually.
        let byte_start = position / 8;
        let bit_offset = position % 8;
        let byte_count = (bit_offset + count).div_ceil(8);
        let mut window = 0_u64;
        for (index, &byte) in self.data[byte_start..byte_start + byte_count]
            .iter()
            .enumerate()
        {
            window |= u64::from(byte) << (index * 8);
        }
        let mask = if count == 32 {
            u64::from(u32::MAX)
        } else {
            (1_u64 << count) - 1
        };
        debug_assert_eq!(position + count, end);
        Ok(((window >> bit_offset) & mask) as u32)
    }

    #[inline]
    fn end_position(&self, position: usize, count: u8) -> Result<usize, DecodeError> {
        if count > 32 {
            return Err(DecodeError::new(
                DecodeErrorKind::InvalidParameter,
                Some(position / 8),
                "cannot read more than 32 bits",
            ));
        }
        let count = usize::from(count);
        let end = position.checked_add(count).ok_or_else(|| {
            DecodeError::new(
                DecodeErrorKind::UnexpectedEof,
                Some(position / 8),
                "bit position overflow",
            )
        })?;
        if end > self.bit_len {
            return Err(DecodeError::new(
                DecodeErrorKind::UnexpectedEof,
                Some(position / 8),
                "truncated bitstream",
            ));
        }
        Ok(end)
    }
}

/// An LSB-first shift register for dense entropy decoding.
///
/// This adapter amortizes slice access and cursor arithmetic across many
/// small reads. It remains fully bounds checked and reports EOF before
/// advancing past the physical input.
#[derive(Debug)]
pub struct ShiftedBitReader<'reader, 'data> {
    reader: &'reader mut BitReader<'data>,
    buffer: u64,
    nbits: u8,
    next_byte_position: usize,
    bit_position: usize,
}

impl ShiftedBitReader<'_, '_> {
    /// Refills the register to at least 56 bits when enough input remains.
    #[inline]
    pub fn fill(&mut self) {
        if self.nbits >= 56 {
            return;
        }

        let remaining = &self.reader.data[self.next_byte_position..];
        if let Some(bytes) = remaining.get(..8) {
            let lookahead = u64::from_le_bytes(bytes.try_into().expect("eight-byte shift window"));
            let byte_count = usize::from((63 - self.nbits) / 8);
            self.buffer |= lookahead << self.nbits;
            self.nbits += (byte_count * 8) as u8;
            self.next_byte_position += byte_count;
            return;
        }

        for &byte in remaining {
            if self.nbits > 56 {
                break;
            }
            self.buffer |= u64::from(byte) << self.nbits;
            self.nbits += 8;
            self.next_byte_position += 1;
        }
    }

    /// Returns the buffered lookahead without consuming it.
    #[must_use]
    #[inline]
    pub const fn peek_full(&self) -> u64 {
        self.buffer
    }

    /// Number of physical input bits currently buffered.
    #[must_use]
    #[inline]
    pub const fn available_bits(&self) -> u8 {
        self.nbits
    }

    /// Consumes already-buffered bits.
    #[inline]
    pub fn consume(&mut self, count: u8) -> Result<(), DecodeError> {
        if count > 32 {
            return Err(DecodeError::new(
                DecodeErrorKind::InvalidParameter,
                Some(self.bit_position / 8),
                "cannot consume more than 32 bits",
            ));
        }
        if self.nbits < count {
            return Err(DecodeError::new(
                DecodeErrorKind::UnexpectedEof,
                Some(self.bit_position / 8),
                "truncated bitstream",
            ));
        }
        self.buffer >>= count;
        self.nbits -= count;
        self.bit_position += usize::from(count);
        Ok(())
    }

    /// Reads up to 32 bits, refilling first when necessary.
    #[inline]
    pub fn read_bits(&mut self, count: u8) -> Result<u32, DecodeError> {
        if count > 32 {
            return Err(DecodeError::new(
                DecodeErrorKind::InvalidParameter,
                Some(self.bit_position / 8),
                "cannot read more than 32 bits",
            ));
        }
        if self.nbits < count {
            self.fill();
        }
        let mask = if count == 32 {
            u64::from(u32::MAX)
        } else if count == 0 {
            0
        } else {
            (1_u64 << count) - 1
        };
        let value = (self.buffer & mask) as u32;
        self.consume(count)?;
        Ok(value)
    }
}

impl Drop for ShiftedBitReader<'_, '_> {
    fn drop(&mut self) {
        self.reader.bit_position = self.bit_position;
        self.reader.window_valid = false;
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
    fn peek_does_not_advance_cursor() {
        let mut reader = BitReader::new(&[0b0101_1010]);
        assert_eq!(reader.peek_bits(4), Ok(0b1010));
        assert_eq!(reader.bit_position(), 0);
        assert_eq!(reader.skip_bits(3), Ok(()));
        assert_eq!(reader.peek_bits(4), Ok(0b1011));
        assert_eq!(reader.bit_position(), 3);
    }

    #[test]
    fn rewind_restores_a_speculative_read_and_rejects_bad_widths() {
        let mut reader = BitReader::new(&[0b0101_1010]);
        assert_eq!(reader.read_bits(5), Ok(0b1_1010));
        assert_eq!(reader.rewind_bits(3), Ok(()));
        assert_eq!(reader.bit_position(), 2);
        assert_eq!(reader.read_bits(3), Ok(0b110));
        let cursor = reader.bit_position();
        assert_eq!(
            reader.rewind_bits(33).unwrap_err().kind(),
            DecodeErrorKind::InvalidParameter
        );
        assert_eq!(reader.bit_position(), cursor);
        assert_eq!(
            reader.rewind_bits(6).unwrap_err().kind(),
            DecodeErrorKind::InvalidParameter
        );
        assert_eq!(reader.bit_position(), cursor);
    }

    #[test]
    fn cached_window_matches_slow_model_across_refills_and_reloads() {
        let data = [
            0x13, 0xa7, 0x5c, 0xe1, 0x09, 0xff, 0x42, 0x81, 0x36, 0xc8, 0x7d, 0x2a,
        ];
        let mut reader = BitReader::new(&data);
        for count in [7, 13, 19, 5, 24, 8] {
            let position = reader.bit_position();
            assert_eq!(
                reader.read_bits(count),
                Ok(slow_read(&data, position, count).unwrap())
            );
        }

        assert_eq!(reader.bit_position(), 76);
        assert_eq!(reader.rewind_bits(20), Ok(()));
        assert_eq!(reader.bit_position(), 56);
        assert_eq!(reader.read_bits(32), Ok(slow_read(&data, 56, 32).unwrap()));
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

    #[test]
    fn shifted_reader_matches_strict_reads_across_offsets_and_refills() {
        let data = [
            0x13, 0xa7, 0x5c, 0xe1, 0x09, 0xff, 0x42, 0x81, 0x36, 0xc8, 0x7d, 0x2a, 0xb4, 0x60,
            0x9e, 0x31, 0x55,
        ];
        for start in [0, 1, 3, 7, 8, 11, 31, 63, 95, 127] {
            let mut strict = BitReader::with_bit_position(&data, start).unwrap();
            let mut shifted_owner = BitReader::with_bit_position(&data, start).unwrap();
            {
                let mut shifted = shifted_owner.shifted();
                for width in [1, 7, 13, 3, 15, 8, 2, 19, 5, 11, 4, 16] {
                    if strict.remaining_bits() < usize::from(width) {
                        break;
                    }
                    assert_eq!(shifted.read_bits(width), strict.read_bits(width));
                }
            }
            assert_eq!(shifted_owner.bit_position(), strict.bit_position());
            assert_eq!(
                shifted_owner.remaining_bits(),
                strict.remaining_bits(),
                "start={start}"
            );
        }
    }

    #[test]
    fn shifted_reader_reports_tail_eof_without_overadvancing() {
        let mut owner = BitReader::with_bit_position(&[0b1011_0101], 3).unwrap();
        {
            let mut shifted = owner.shifted();
            assert_eq!(shifted.read_bits(5), Ok(0b10110));
            assert_eq!(
                shifted.read_bits(1).unwrap_err().kind(),
                DecodeErrorKind::UnexpectedEof
            );
        }
        assert_eq!(owner.bit_position(), 8);
        assert_eq!(owner.remaining_bits(), 0);
    }
}
