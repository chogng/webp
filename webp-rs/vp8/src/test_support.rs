//! Shared test-only VP8 bitstream fixtures.

use crate::CoefficientBlockType;
use crate::CoefficientProbabilities;
use crate::coefficients::COEFFICIENT_UPDATE_PROBABILITIES;
use crate::partition::KEY_FRAME_HEADER_LEN;
use crate::partition::KEY_FRAME_START_CODE;

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
            let shift = 7 - (self.range + 1).ilog2() as i32;
            self.range = ((self.range + 1) << shift) - 1;
            self.value <<= shift;
            self.pending_bits += shift;
            if self.pending_bits > 0 {
                self.flush();
            }
        }
    }

    pub(crate) fn write_literal(&mut self, value: u32, count: u8) {
        for shift in (0..count).rev() {
            self.write_bool(((value >> shift) & 1) != 0, 128);
        }
    }

    pub(crate) fn write_signed_literal(&mut self, value: i32, count: u8) {
        self.write_literal(value.unsigned_abs(), count);
        self.write_bool(value.is_negative(), 128);
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

pub(crate) fn write_quantization_header(
    writer: &mut TestBoolWriter,
    base_index: u8,
    deltas: [i32; 5],
    refresh_entropy_probabilities: bool,
) {
    writer.write_literal(u32::from(base_index), 7);
    for value in deltas {
        writer.write_bool(value != 0, 128);
        if value != 0 {
            writer.write_signed_literal(value, 4);
        }
    }
    writer.write_bool(refresh_entropy_probabilities, 128);
}

pub(crate) fn write_coefficient_updates(
    writer: &mut TestBoolWriter,
    updates: &[(usize, usize, usize, usize, u8)],
    use_skip_probability: bool,
    skip_probability: u8,
) {
    for (coefficient_type, bands) in COEFFICIENT_UPDATE_PROBABILITIES.iter().enumerate() {
        for (band, contexts) in bands.iter().enumerate() {
            for (context, nodes) in contexts.iter().enumerate() {
                for (node, &update_probability) in nodes.iter().enumerate() {
                    let update = updates.iter().find(|&&(t, b, c, n, _)| {
                        (t, b, c, n) == (coefficient_type, band, context, node)
                    });
                    writer.write_bool(update.is_some(), update_probability);
                    if let Some(&(_, _, _, _, value)) = update {
                        writer.write_literal(u32::from(value), 8);
                    }
                }
            }
        }
    }
    writer.write_bool(use_skip_probability, 128);
    if use_skip_probability {
        writer.write_literal(u32::from(skip_probability), 8);
    }
}

pub(crate) fn pad_first_partition(writer: &mut TestBoolWriter) {
    writer.write_literal(0, 8);
}

pub(crate) fn coefficient_nodes(
    probabilities: &CoefficientProbabilities,
    coefficient_type: CoefficientBlockType,
    position: usize,
    context: usize,
) -> &[u8; 11] {
    probabilities.nodes(coefficient_type, position, context)
}

pub(crate) fn write_coefficient_eob(
    writer: &mut TestBoolWriter,
    probabilities: &CoefficientProbabilities,
    coefficient_type: CoefficientBlockType,
    position: usize,
    context: usize,
) {
    writer.write_bool(
        false,
        coefficient_nodes(probabilities, coefficient_type, position, context)[0],
    );
}

pub(crate) fn key_frame(
    width: u16,
    height: u16,
    version: u8,
    show_frame: bool,
    partition_len: u32,
) -> [u8; KEY_FRAME_HEADER_LEN] {
    let tag = (partition_len << 5) | (u32::from(show_frame) << 4) | (u32::from(version) << 1);
    let mut payload = [0_u8; KEY_FRAME_HEADER_LEN];
    payload[..3].copy_from_slice(&tag.to_le_bytes()[..3]);
    payload[3..6].copy_from_slice(&KEY_FRAME_START_CODE);
    payload[6..8].copy_from_slice(&width.to_le_bytes());
    payload[8..10].copy_from_slice(&height.to_le_bytes());
    payload
}
