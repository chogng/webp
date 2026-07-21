//! Shared test-only VP8 bitstream fixtures.

/// A straightforward VP8 boolean writer for decoder test vectors.
#[derive(Default)]
pub(crate) struct TestBoolWriter {
    range: i32,
    value: i32,
    run: usize,
    pending_bits: i32,
    bytes: Vec<u8>,
}

impl TestBoolWriter {
    pub(crate) fn new() -> Self {
        Self {
            range: 254,
            value: 0,
            run: 0,
            pending_bits: -8,
            bytes: Vec::new(),
        }
    }

    pub(crate) fn write_bool(&mut self, bit: bool, probability: u8) {
        let split = (self.range * i32::from(probability)) >> 8;
        if bit {
            self.value += split + 1;
            self.range -= split + 1;
        } else {
            self.range = split;
        }
        if self.range < 127 {
            let shift = if self.range == 0 {
                7
            } else {
                7 - self.range.ilog2() as i32
            };
            self.range = ((self.range + 1) << shift) - 1;
            self.value <<= shift;
            self.pending_bits += shift;
            if self.pending_bits > 0 {
                self.flush();
            }
        }
    }

    pub(crate) fn finish(mut self) -> Vec<u8> {
        for _ in (0..(9 - self.pending_bits) as u8).rev() {
            self.write_bool(false, 128);
        }
        self.pending_bits = 0;
        self.flush();
        self.bytes
    }

    fn flush(&mut self) {
        let shift = 8 + self.pending_bits;
        let bits = self.value >> shift;
        self.value -= bits << shift;
        self.pending_bits -= 8;
        if bits & 0xff == 0xff {
            self.run += 1;
            return;
        }
        if bits & 0x100 != 0
            && let Some(previous) = self.bytes.last_mut()
        {
            *previous += 1;
        }
        let delayed = if bits & 0x100 != 0 { 0 } else { 0xff };
        self.bytes.extend(std::iter::repeat_n(delayed, self.run));
        self.run = 0;
        self.bytes.push((bits & 0xff) as u8);
    }
}
