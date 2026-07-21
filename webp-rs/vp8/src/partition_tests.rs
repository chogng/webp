use super::*;
use crate::BoolDecoder;
use crate::test_support::TestBoolWriter;
use crate::test_support::key_frame;
use crate::test_support::pad_first_partition;
use crate::test_support::write_coefficient_updates;
use crate::test_support::write_quantization_header;
use webp_core::DecodeErrorKind;
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

#[test]
fn parses_key_frame_dimensions_tag_and_scale_bits() {
    let payload = key_frame(0x800d, 0xc009, 3, true, 0);
    let header = parse_riff_payload(&payload, Some((13, 9)), &DecodeLimits::default()).unwrap();
    assert_eq!(header.width, 13);
    assert_eq!(header.height, 9);
    assert_eq!(header.version, 3);
    assert_eq!(header.first_partition_len, 0);
    assert_eq!(header.horizontal_scale, 2);
    assert_eq!(header.vertical_scale, 3);
}

#[test]
fn rejects_all_fixed_header_truncations() {
    let payload = key_frame(1, 1, 0, true, 0);
    for end in 0..KEY_FRAME_HEADER_LEN {
        assert_eq!(
            parse_riff_payload(&payload[..end], None, &DecodeLimits::default())
                .unwrap_err()
                .kind(),
            DecodeErrorKind::UnexpectedEof,
            "truncation at {end}",
        );
    }
}

#[test]
fn rejects_invalid_tag_signature_dimensions_partition_and_canvas() {
    let limits = DecodeLimits::default();
    let mut inter = key_frame(1, 1, 0, true, 0);
    inter[0] |= 1;
    assert_eq!(
        parse_riff_payload(&inter, None, &limits)
            .unwrap_err()
            .kind(),
        DecodeErrorKind::UnsupportedFeature
    );

    let invisible = key_frame(1, 1, 0, false, 0);
    assert_eq!(
        parse_riff_payload(&invisible, None, &limits)
            .unwrap_err()
            .kind(),
        DecodeErrorKind::InvalidBitstream
    );

    let unsupported_version = key_frame(1, 1, 4, true, 0);
    assert_eq!(
        parse_riff_payload(&unsupported_version, None, &limits)
            .unwrap_err()
            .kind(),
        DecodeErrorKind::InvalidBitstream
    );

    let mut bad_signature = key_frame(1, 1, 0, true, 0);
    bad_signature[5] ^= 1;
    assert_eq!(
        parse_riff_payload(&bad_signature, None, &limits)
            .unwrap_err()
            .kind(),
        DecodeErrorKind::InvalidBitstream
    );

    let zero_width = key_frame(0, 1, 0, true, 0);
    assert_eq!(
        parse_riff_payload(&zero_width, None, &limits)
            .unwrap_err()
            .kind(),
        DecodeErrorKind::InvalidBitstream
    );

    let partition_past_end = key_frame(1, 1, 0, true, 1);
    assert_eq!(
        parse_riff_payload(&partition_past_end, None, &limits)
            .unwrap_err()
            .kind(),
        DecodeErrorKind::UnexpectedEof
    );

    let valid = key_frame(1, 1, 0, true, 0);
    assert_eq!(
        parse_riff_payload(&valid, Some((2, 1)), &limits)
            .unwrap_err()
            .kind(),
        DecodeErrorKind::InvalidContainer
    );
}

#[test]
fn enforces_image_limits_before_decoder_state_is_created() {
    let payload = key_frame(8, 1, 0, true, 0);
    let limits = DecodeLimits {
        max_width: 7,
        ..DecodeLimits::default()
    };
    assert_eq!(
        parse_riff_payload(&payload, None, &limits)
            .unwrap_err()
            .kind(),
        DecodeErrorKind::LimitExceeded
    );
}

#[test]
fn parses_first_partition_controls_and_four_token_partitions() {
    let mut writer = TestBoolWriter::new();
    writer.write_bool(false, 128); // colour space
    writer.write_bool(true, 128); // clamp type
    writer.write_bool(true, 128); // segmentation enabled
    writer.write_bool(true, 128); // update segment map
    writer.write_bool(true, 128); // update segment data
    writer.write_bool(false, 128); // delta rather than absolute values
    for value in [-5, 0, 3, 0] {
        writer.write_bool(value != 0, 128);
        if value != 0 {
            writer.write_signed_literal(value, 7);
        }
    }
    for value in [-4, 0, 0, 0] {
        writer.write_bool(value != 0, 128);
        if value != 0 {
            writer.write_signed_literal(value, 6);
        }
    }
    for value in [11_u8, 255, 77] {
        writer.write_bool(value != 255, 128);
        if value != 255 {
            writer.write_literal(u32::from(value), 8);
        }
    }
    writer.write_bool(false, 128); // normal filter
    writer.write_literal(17, 6);
    writer.write_literal(4, 3);
    writer.write_bool(true, 128); // loop-filter deltas enabled
    writer.write_bool(true, 128); // update deltas
    for value in [2, 0, 0, 0] {
        writer.write_bool(value != 0, 128);
        if value != 0 {
            writer.write_signed_literal(value, 6);
        }
    }
    for value in [0, 0, 0, -1] {
        writer.write_bool(value != 0, 128);
        if value != 0 {
            writer.write_signed_literal(value, 6);
        }
    }
    writer.write_literal(2, 2); // four coefficient-token partitions
    write_quantization_header(&mut writer, 63, [-7, 0, 4, 0, -3], false);
    write_coefficient_updates(&mut writer, &[], false, 0);
    pad_first_partition(&mut writer);
    let mut partition_zero = writer.finish();
    partition_zero.extend_from_slice(&[0; 8]);

    let mut payload = key_frame(3, 5, 0, true, partition_zero.len() as u32).to_vec();
    payload.extend_from_slice(&partition_zero);
    payload.extend_from_slice(&[1, 0, 0, 2, 0, 0, 0, 0, 0]);
    payload.extend_from_slice(&[0xaa, 0xbb, 0xcc, 0xdd]);
    let frame = parse_riff_payload(&payload, None, &DecodeLimits::default()).unwrap();
    let layout = parse_partition_layout(&payload, &frame, &DecodeLimits::default()).unwrap();

    assert!(!layout.header.colorspace_reserved);
    assert!(layout.header.clamp_type);
    assert_eq!(layout.header.token_partition_count, 4);
    assert_eq!(layout.header.segments.quantizer, [-5, 0, 3, 0]);
    assert_eq!(layout.header.segments.filter_strength, [-4, 0, 0, 0]);
    assert_eq!(layout.header.segments.probabilities, [11, 255, 77]);
    assert_eq!(layout.header.filter.level, 17);
    assert_eq!(layout.header.filter.sharpness, 4);
    assert_eq!(layout.header.filter.ref_deltas, [2, 0, 0, 0]);
    assert_eq!(layout.header.filter.mode_deltas, [0, 0, 0, -1]);
    assert_eq!(
        layout.header.quantization,
        QuantizationHeader {
            base_index: 63,
            y1_dc_delta: -7,
            y2_dc_delta: 0,
            y2_ac_delta: 4,
            uv_dc_delta: 0,
            uv_ac_delta: -3,
        }
    );
    assert!(!layout.header.refresh_entropy_probabilities);
    assert_eq!(layout.header.coefficients.get(0, 0, 0, 0), 128);
    assert_eq!(layout.header.coefficients.get(0, 1, 0, 0), 253);
    assert_eq!(layout.header.coefficients.get(3, 7, 2, 10), 128);
    assert!(!layout.header.coefficients.use_skip_probability);
    assert_eq!(layout.header.coefficients.skip_probability, 0);
    assert_eq!(
        layout
            .tokens
            .iter()
            .map(|part| part.data)
            .collect::<Vec<_>>(),
        vec![&[0xaa][..], &[0xbb, 0xcc], &[], &[0xdd]],
    );
}

#[test]
fn rejects_truncated_or_oversized_token_partition_tables() {
    let mut writer = TestBoolWriter::new();
    writer.write_bool(false, 128); // colour space
    writer.write_bool(false, 128); // clamp type
    writer.write_bool(false, 128); // no segmentation
    writer.write_bool(false, 128); // normal filter
    writer.write_literal(0, 6);
    writer.write_literal(0, 3);
    writer.write_bool(false, 128); // no filter deltas
    writer.write_literal(2, 2); // four token partitions
    write_quantization_header(&mut writer, 0, [0; 5], false);
    write_coefficient_updates(&mut writer, &[], false, 0);
    pad_first_partition(&mut writer);
    let partition_zero = writer.finish();
    let mut payload = key_frame(1, 1, 0, true, partition_zero.len() as u32).to_vec();
    payload.extend_from_slice(&partition_zero);
    let frame = parse_riff_payload(&payload, None, &DecodeLimits::default()).unwrap();
    assert_eq!(
        parse_partition_layout(&payload, &frame, &DecodeLimits::default())
            .unwrap_err()
            .kind(),
        DecodeErrorKind::UnexpectedEof
    );

    payload.extend_from_slice(&[5, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    assert_eq!(
        parse_partition_layout(&payload, &frame, &DecodeLimits::default())
            .unwrap_err()
            .kind(),
        DecodeErrorKind::UnexpectedEof
    );
}

#[test]
fn parses_each_legal_token_partition_count() {
    for partition_bits in 0..4_u32 {
        let mut writer = TestBoolWriter::new();
        writer.write_bool(false, 128); // colour space
        writer.write_bool(false, 128); // clamp type
        writer.write_bool(false, 128); // no segmentation
        writer.write_bool(false, 128); // normal filter
        writer.write_literal(0, 6);
        writer.write_literal(0, 3);
        writer.write_bool(false, 128); // no filter deltas
        writer.write_literal(partition_bits, 2);
        write_quantization_header(&mut writer, 0, [0; 5], false);
        write_coefficient_updates(&mut writer, &[], false, 0);
        pad_first_partition(&mut writer);
        let partition_zero = writer.finish();
        let partition_count = 1_usize << partition_bits;
        let mut payload = key_frame(1, 1, 0, true, partition_zero.len() as u32).to_vec();
        payload.extend_from_slice(&partition_zero);
        payload.resize(payload.len() + 3 * (partition_count - 1), 0);
        payload.push(0);

        let frame = parse_riff_payload(&payload, None, &DecodeLimits::default()).unwrap();
        let layout = parse_partition_layout(&payload, &frame, &DecodeLimits::default()).unwrap();
        assert_eq!(
            layout.header.token_partition_count as usize,
            partition_count
        );
        assert_eq!(layout.tokens.len(), partition_count);
        assert_eq!(layout.tokens.last().unwrap().data, &[0]);
    }
}
