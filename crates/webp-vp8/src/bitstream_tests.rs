use super::*;

/// A deliberately straightforward VP8 boolean writer used only to produce
/// independently driven decoder vectors. It follows the encoder interval
/// update and byte-flush rules, not the decoder's cached-value structure.
#[derive(Default)]
struct TestBoolWriter {
    range: i32,
    value: i32,
    run: usize,
    pending_bits: i32,
    bytes: Vec<u8>,
}

impl TestBoolWriter {
    fn new() -> Self {
        Self {
            range: 254,
            value: 0,
            run: 0,
            pending_bits: -8,
            bytes: Vec::new(),
        }
    }

    fn write_bool(&mut self, bit: bool, probability: u8) {
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

    fn write_literal(&mut self, value: u32, count: u8) {
        for shift in (0..count).rev() {
            self.write_bool(((value >> shift) & 1) != 0, 128);
        }
    }

    fn write_signed_literal(&mut self, value: i32, count: u8) {
        self.write_literal(value.unsigned_abs(), count);
        self.write_bool(value.is_negative(), 128);
    }

    fn finish(mut self) -> Vec<u8> {
        self.write_literal(0, (9 - self.pending_bits) as u8);
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

#[test]
fn boolean_decoder_recovers_mixed_probability_vectors() {
    let probabilities = [1_u8, 2, 127, 128, 254, 1, 128, 254, 2];
    let expected = [true, false, true, true, false, false, true, true, false];
    let mut writer = TestBoolWriter::new();
    for (&bit, &probability) in expected.iter().zip(probabilities.iter()) {
        writer.write_bool(bit, probability);
    }
    let bytes = writer.finish();
    assert!(!bytes.is_empty());

    let mut decoder = BoolDecoder::new(&bytes, &DecodeLimits::default()).unwrap();
    for (index, (&bit, &probability)) in expected.iter().zip(probabilities.iter()).enumerate() {
        assert_eq!(
            decoder.read_bool(probability).unwrap(),
            bit,
            "symbol {index}"
        );
    }
    assert_eq!(
        decoder.remaining_work(),
        DecodeLimits::default().max_work_units - expected.len() as u64
    );
    assert!(decoder.bytes_consumed() <= bytes.len());
}

#[test]
fn production_boolean_encoder_round_trips_mixed_probability_vectors() {
    let probabilities = [1_u8, 2, 127, 128, 254, 1, 128, 254, 2];
    let expected = [true, false, true, true, false, false, true, true, false];
    let mut encoder = BoolEncoder::new();
    for (&bit, &probability) in expected.iter().zip(probabilities.iter()) {
        encoder.write_bool(bit, probability).unwrap();
    }
    encoder.write_literal(0x1234, 16).unwrap();
    encoder.write_signed_literal(-17, 7).unwrap();
    let bytes = encoder.finish().unwrap();
    let mut decoder = BoolDecoder::new(&bytes, &DecodeLimits::default()).unwrap();
    for (&bit, &probability) in expected.iter().zip(probabilities.iter()) {
        assert_eq!(decoder.read_bool(probability).unwrap(), bit);
    }
    assert_eq!(decoder.read_literal(16).unwrap(), 0x1234);
    assert_eq!(decoder.read_signed_literal(7).unwrap(), -17);
}

#[test]
fn production_boolean_encoder_round_trips_long_category_probability_vectors() {
    let probabilities = crate::coefficients::CATEGORY_PROBABILITIES[3];
    let expected = [
        false, true, false, true, false, false, false, true, true, false, true,
    ];
    let mut encoder = BoolEncoder::new();
    for (&bit, &probability) in expected.iter().zip(probabilities) {
        encoder.write_bool(bit, probability).unwrap();
    }
    let bytes = encoder.finish().unwrap();
    let mut decoder = BoolDecoder::new(&bytes, &DecodeLimits::default()).unwrap();
    for (index, (&bit, &probability)) in expected.iter().zip(probabilities).enumerate() {
        assert_eq!(decoder.read_bool(probability).unwrap(), bit, "symbol {index}");
    }
}

#[test]
fn boolean_decoder_handles_extreme_probabilities() {
    let mut true_values = BoolDecoder::new(&[0xff], &DecodeLimits::default()).unwrap();
    assert_eq!(true_values.read_bool(0), Ok(true));
    assert_eq!(true_values.read_bool(255), Ok(true));

    let mut false_value = BoolDecoder::new(&[0], &DecodeLimits::default()).unwrap();
    assert_eq!(false_value.read_bool(255), Ok(false));
}

#[test]
fn boolean_decoder_reads_msb_first_literals() {
    let mut writer = TestBoolWriter::new();
    writer.write_literal(0b10110, 5);
    writer.write_literal(0x1234, 16);
    writer.write_signed_literal(-17, 7);
    writer.write_bool(false, 128); // Keep the signed value away from EOF.
    let bytes = writer.finish();
    let mut decoder = BoolDecoder::new(&bytes, &DecodeLimits::default()).unwrap();
    assert_eq!(decoder.read_literal(5), Ok(0b10110));
    assert_eq!(decoder.read_literal(16), Ok(0x1234));
    assert_eq!(decoder.read_signed_literal(7), Ok(-17));
    assert_eq!(
        decoder.read_literal(33).unwrap_err().kind(),
        DecodeErrorKind::InvalidParameter
    );
}

#[test]
fn boolean_decoder_reports_eof_and_work_budget_exhaustion() {
    let mut empty = BoolDecoder::new(&[], &DecodeLimits::default()).unwrap();
    assert_eq!(
        empty.read_bool(128).unwrap_err().kind(),
        DecodeErrorKind::UnexpectedEof
    );

    let limited = DecodeLimits {
        max_work_units: 1,
        ..DecodeLimits::default()
    };
    let mut decoder = BoolDecoder::new(&[0], &limited).unwrap();
    assert!(decoder.read_bool(128).is_ok());
    assert_eq!(
        decoder.read_bool(128).unwrap_err().kind(),
        DecodeErrorKind::LimitExceeded
    );
}

#[test]
fn boolean_decoder_private_cache_tracks_loaded_byte() {
    let mut decoder = BoolDecoder::new(&[0xa5], &DecodeLimits::default()).unwrap();
    decoder.load_byte().unwrap();
    assert_eq!(decoder.byte_position, 1);
    assert_eq!(decoder.value, 0xa5);
    assert_eq!(decoder.bits, 0);
}
