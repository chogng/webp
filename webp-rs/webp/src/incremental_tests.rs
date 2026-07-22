use super::*;

#[cfg(feature = "encode")]
fn patterned_rgba(width: u32, height: u32, alpha: bool) -> Vec<u8> {
    let mut rgba = Vec::new();
    for y in 0..height {
        for x in 0..width {
            rgba.extend_from_slice(&[
                (x * 17 + y * 3) as u8,
                (x * 5 + y * 11) as u8,
                (x * 7 + y * 13) as u8,
                if alpha { (x * 9 + y * 19) as u8 } else { 255 },
            ]);
        }
    }
    rgba
}

#[cfg(feature = "encode")]
fn assert_all_single_splits(bytes: &[u8]) {
    let expected = crate::decode(bytes, &DecodeOptions::default()).unwrap();
    for split in 0..=bytes.len() {
        let mut decoder = IncrementalDecoder::new(DecodeOptions::default());
        let first = decoder.push(&bytes[..split]).unwrap();
        if first != Progress::Complete {
            decoder.push(&[]).unwrap();
            decoder.push(&bytes[split..]).unwrap();
        }
        assert_eq!(decoder.finish().unwrap(), expected, "split={split}");
    }
}

#[cfg(feature = "encode")]
#[test]
fn every_single_split_matches_one_shot_for_static_codecs_and_alpha() {
    let lossless_rgba = patterned_rgba(3, 5, true);
    let lossless = crate::encode_lossless_rgba(3, 5, &lossless_rgba).unwrap();
    assert_all_single_splits(&lossless);

    let lossy_rgba = patterned_rgba(17, 33, false);
    let lossy = crate::encode_lossy_rgba(17, 33, &lossy_rgba).unwrap();
    assert_all_single_splits(&lossy);

    let alpha_rgba = patterned_rgba(17, 33, true);
    let alpha = crate::encode_lossy_rgba(17, 33, &alpha_rgba).unwrap();
    assert_all_single_splits(&alpha);
}

#[cfg(feature = "encode")]
#[test]
fn one_byte_pushes_publish_monotonic_rows_and_finish_equivalently() {
    let rgba = patterned_rgba(32, 64, false);
    let bytes = crate::encode_lossy_rgba(32, 64, &rgba).unwrap();
    let expected = crate::decode(&bytes, &DecodeOptions::default()).unwrap();
    let mut decoder = IncrementalDecoder::new(DecodeOptions::default());
    let mut previous = 0;
    let mut saw_partial = false;
    for (offset, byte) in bytes.iter().enumerate() {
        let progress = decoder.push(std::slice::from_ref(byte)).unwrap();
        if let Some(view) = decoder.decoded() {
            assert!(view.decoded_rows >= previous);
            assert_eq!(
                view.rgba.len(),
                view.width as usize * view.decoded_rows as usize * 4
            );
            assert_eq!(view.rgba, &expected.rgba[..view.rgba.len()]);
            saw_partial |= view.decoded_rows != 0 && view.decoded_rows < view.height;
            previous = view.decoded_rows;
        }
        if offset + 1 == bytes.len() {
            assert_eq!(progress, Progress::Complete);
        }
    }
    assert!(
        saw_partial,
        "the VP8 stream never exposed a stable partial row prefix"
    );
    assert_eq!(decoder.finish().unwrap(), expected);
}

#[cfg(feature = "encode")]
#[test]
fn every_proper_prefix_finishes_as_truncated() {
    let rgba = patterned_rgba(17, 33, true);
    let fixtures = [
        crate::encode_lossless_rgba(17, 33, &rgba).unwrap(),
        crate::encode_lossy_rgba(17, 33, &rgba).unwrap(),
    ];
    for bytes in fixtures {
        for end in 0..bytes.len() {
            let mut decoder = IncrementalDecoder::new(DecodeOptions::default());
            decoder.push(&bytes[..end]).unwrap();
            assert_eq!(
                decoder.finish().unwrap_err().kind(),
                DecodeErrorKind::UnexpectedEof,
                "prefix={end}"
            );
        }
    }
}

#[cfg(feature = "encode")]
#[test]
fn resource_failures_match_one_shot() {
    let rgba = patterned_rgba(17, 33, false);
    let bytes = crate::encode_lossy_rgba(17, 33, &rgba).unwrap();
    for limits in [
        crate::DecodeLimits {
            max_alloc_bytes: 32,
            ..crate::DecodeLimits::default()
        },
        crate::DecodeLimits {
            max_work_units: 1,
            ..crate::DecodeLimits::default()
        },
    ] {
        let options = DecodeOptions {
            limits,
            ..DecodeOptions::default()
        };
        let expected = crate::decode(&bytes, &options).unwrap_err().kind();
        let mut decoder = IncrementalDecoder::new(options);
        let actual = decoder
            .push(&bytes)
            .map(|_| decoder.finish())
            .unwrap_or_else(Err)
            .unwrap_err()
            .kind();
        assert_eq!(actual, expected);
    }
}

#[cfg(feature = "encode")]
#[test]
fn info_arrives_before_pixels_and_complete_is_terminal() {
    let rgba = patterned_rgba(1, 1, true);
    let bytes = crate::encode_lossless_rgba(1, 1, &rgba).unwrap();
    let payload_start = bytes
        .windows(4)
        .position(|window| window == b"VP8L")
        .unwrap()
        + 8;
    let mut decoder = IncrementalDecoder::new(DecodeOptions::default());
    assert_eq!(
        decoder.push(&bytes[..payload_start + 5]).unwrap(),
        Progress::NeedMoreData
    );
    assert_eq!(decoder.info().unwrap().width, 1);
    assert!(decoder.decoded().is_none());
    assert_eq!(
        decoder.push(&bytes[payload_start + 5..]).unwrap(),
        Progress::Complete
    );
    assert_eq!(
        decoder.push(&[]).unwrap_err().kind(),
        DecodeErrorKind::InvalidParameter
    );
}

#[test]
fn declared_input_and_pixel_limits_fail_before_codec_allocation() {
    let mut bytes = b"RIFF".to_vec();
    bytes.extend_from_slice(&100_u32.to_le_bytes());
    bytes.extend_from_slice(b"WEBP");
    let mut decoder = IncrementalDecoder::new(DecodeOptions {
        limits: crate::DecodeLimits {
            max_input_bytes: 11,
            ..crate::DecodeLimits::default()
        },
        ..DecodeOptions::default()
    });
    assert_eq!(
        decoder.push(&bytes).unwrap_err().kind(),
        DecodeErrorKind::LimitExceeded
    );
    assert_eq!(
        decoder.push(&[]).unwrap_err().kind(),
        DecodeErrorKind::InvalidParameter
    );
}

#[test]
fn container_metadata_and_canvas_limits_fail_while_headers_advance() {
    fn riff(body: Vec<u8>) -> Vec<u8> {
        let mut bytes = b"RIFF".to_vec();
        bytes.extend_from_slice(&(body.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&body);
        bytes
    }
    fn push_chunk(output: &mut Vec<u8>, fourcc: [u8; 4], payload: &[u8]) {
        output.extend_from_slice(&fourcc);
        output.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        output.extend_from_slice(payload);
        if payload.len() % 2 == 1 {
            output.push(0);
        }
    }

    let mut metadata_body = b"WEBP".to_vec();
    push_chunk(&mut metadata_body, webp_container::EXIF, &[1, 2]);
    push_chunk(&mut metadata_body, webp_container::XMP, &[3, 4]);
    let mut decoder = IncrementalDecoder::new(DecodeOptions {
        limits: crate::DecodeLimits {
            max_metadata_bytes: 3,
            ..crate::DecodeLimits::default()
        },
        ..DecodeOptions::default()
    });
    assert_eq!(
        decoder.push(&riff(metadata_body)).unwrap_err().kind(),
        DecodeErrorKind::LimitExceeded
    );

    let mut canvas_body = b"WEBP".to_vec();
    push_chunk(
        &mut canvas_body,
        webp_container::VP8X,
        &[0, 0, 0, 0, 1, 0, 0, 0, 0, 0],
    );
    let mut decoder = IncrementalDecoder::new(DecodeOptions {
        limits: crate::DecodeLimits {
            max_pixels: 1,
            ..crate::DecodeLimits::default()
        },
        ..DecodeOptions::default()
    });
    assert_eq!(
        decoder.push(&riff(canvas_body)).unwrap_err().kind(),
        DecodeErrorKind::LimitExceeded
    );
}

#[test]
fn animation_is_an_explicit_incremental_error() {
    fn push_chunk(output: &mut Vec<u8>, fourcc: [u8; 4], payload: &[u8]) {
        output.extend_from_slice(&fourcc);
        output.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        output.extend_from_slice(payload);
        if payload.len() % 2 == 1 {
            output.push(0);
        }
    }

    let mut writer = crate::BitWriter::new();
    writer.write_bits(0x2f, 8).unwrap();
    writer.write_bits(0, 14).unwrap();
    writer.write_bits(0, 14).unwrap();
    writer.write_bits(0, 1).unwrap();
    writer.write_bits(0, 3).unwrap();
    writer.write_bits(0, 3).unwrap();
    for channel in [0_u8; 5] {
        writer.write_bits(1, 1).unwrap();
        writer.write_bits(0, 1).unwrap();
        writer.write_bits(1, 1).unwrap();
        writer.write_bits(u32::from(channel), 8).unwrap();
    }
    let mut frame = vec![0; 16];
    push_chunk(&mut frame, webp_container::VP8L, &writer.into_bytes());

    let mut body = b"WEBP".to_vec();
    push_chunk(
        &mut body,
        webp_container::VP8X,
        &[VP8X_ANIMATION_FLAG, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    );
    push_chunk(&mut body, webp_container::ANIM, &[0; 6]);
    push_chunk(&mut body, webp_container::ANMF, &frame);
    let mut bytes = b"RIFF".to_vec();
    bytes.extend_from_slice(&(body.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&body);
    let mut decoder = IncrementalDecoder::new(DecodeOptions::default());
    assert_eq!(
        decoder.push(&bytes).unwrap_err().kind(),
        DecodeErrorKind::UnsupportedFeature
    );
}
