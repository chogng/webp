use super::*;
use crate::BoolDecoder;
use crate::test_support::TestBoolWriter;
use webp_core::DecodeLimits;

#[test]
fn disabled_segmentation_uses_vp8_defaults() {
    let mut writer = TestBoolWriter::new();
    writer.write_bool(false, 128);
    let bytes = writer.finish();
    let mut bits = BoolDecoder::new(&bytes, &DecodeLimits::default()).unwrap();

    assert_eq!(
        parse_segment_header(&mut bits).unwrap(),
        SegmentHeader {
            enabled: false,
            update_map: false,
            absolute_delta: true,
            quantizer: [0; 4],
            filter_strength: [0; 4],
            probabilities: [255; 3],
        }
    );
}
