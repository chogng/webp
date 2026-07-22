use super::*;
use crate::DecodeLimits;
use crate::vp8::CoefficientProbabilities;
use crate::vp8::FilterHeader;
use crate::vp8::QuantizationHeader;
use crate::vp8::SegmentHeader;

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

fn first_partition_header(
    segments: SegmentHeader,
    coefficients: CoefficientProbabilities,
) -> FirstPartitionHeader {
    FirstPartitionHeader {
        colorspace_reserved: false,
        clamp_type: false,
        segments,
        filter: FilterHeader {
            simple: false,
            level: 0,
            sharpness: 0,
            use_deltas: false,
            ref_deltas: [0; 4],
            mode_deltas: [0; 4],
        },
        token_partition_count: 1,
        quantization: QuantizationHeader {
            base_index: 0,
            y1_dc_delta: 0,
            y2_dc_delta: 0,
            y2_ac_delta: 0,
            uv_dc_delta: 0,
            uv_ac_delta: 0,
        },
        refresh_entropy_probabilities: false,
        coefficients,
    }
}

#[test]
fn parses_segments_skip_and_sixteen_by_sixteen_modes() {
    let coefficients = CoefficientProbabilities {
        use_skip_probability: true,
        skip_probability: 128,
        ..CoefficientProbabilities::default()
    };
    let header = first_partition_header(
        SegmentHeader {
            enabled: true,
            update_map: true,
            absolute_delta: true,
            quantizer: [0; 4],
            filter_strength: [0; 4],
            probabilities: [128; 3],
        },
        coefficients,
    );
    let mut writer = TestBoolWriter::new();

    writer.write_bool(false, 128);
    writer.write_bool(true, 128);
    writer.write_bool(true, 128);
    writer.write_bool(true, 145);
    writer.write_bool(false, 156);
    writer.write_bool(true, 163);
    writer.write_bool(true, 142);
    writer.write_bool(false, 114);
    writer.write_bool(true, 128);
    writer.write_bool(false, 128);
    writer.write_bool(false, 128);
    writer.write_bool(true, 145);
    writer.write_bool(true, 156);
    writer.write_bool(false, 128);
    writer.write_bool(true, 142);
    writer.write_bool(true, 114);
    writer.write_bool(true, 183);

    let bytes = writer.finish();
    let mut decoder = BoolDecoder::new(&bytes, &DecodeLimits::default()).unwrap();
    let mut top = [Intra4Mode::Dc; 8];
    let mut blocks = [IntraMacroblock {
        segment: 0,
        skip: false,
        luma: LumaMode::Sixteen(Intra16Mode::Dc),
        chroma: ChromaMode::Dc,
    }; 2];
    parse_intra_mode_row(&mut decoder, &header, &mut top, &mut blocks).unwrap();

    assert_eq!(blocks[0].segment, 1);
    assert!(blocks[0].skip);
    assert_eq!(blocks[0].luma, LumaMode::Sixteen(Intra16Mode::Vertical));
    assert_eq!(blocks[0].chroma, ChromaMode::Vertical);
    assert_eq!(blocks[1].segment, 2);
    assert!(!blocks[1].skip);
    assert_eq!(blocks[1].luma, LumaMode::Sixteen(Intra16Mode::Horizontal));
    assert_eq!(blocks[1].chroma, ChromaMode::TrueMotion);
}

#[test]
fn decodes_four_by_four_modes_and_validates_context_shape() {
    let header = first_partition_header(
        SegmentHeader {
            enabled: false,
            update_map: false,
            absolute_delta: true,
            quantizer: [0; 4],
            filter_strength: [0; 4],
            probabilities: [255; 3],
        },
        CoefficientProbabilities::default(),
    );
    let mut writer = TestBoolWriter::new();
    writer.write_bool(false, 145);
    for _ in 0..16 {
        writer.write_bool(false, 231);
    }
    writer.write_bool(false, 142);

    let bytes = writer.finish();
    let mut decoder = BoolDecoder::new(&bytes, &DecodeLimits::default()).unwrap();
    let mut top = [Intra4Mode::Dc; 4];
    let mut blocks = [IntraMacroblock {
        segment: 0,
        skip: false,
        luma: LumaMode::Sixteen(Intra16Mode::Dc),
        chroma: ChromaMode::Dc,
    }];
    parse_intra_mode_row(&mut decoder, &header, &mut top, &mut blocks).unwrap();
    assert_eq!(blocks[0].luma, LumaMode::FourByFour([Intra4Mode::Dc; 16]));
    assert_eq!(blocks[0].chroma, ChromaMode::Dc);
    assert_eq!(top, [Intra4Mode::Dc; 4]);

    let mut no_top = [];
    assert_eq!(
        parse_intra_mode_row(&mut decoder, &header, &mut no_top, &mut blocks)
            .unwrap_err()
            .kind(),
        DecodeErrorKind::InvalidParameter
    );
}
