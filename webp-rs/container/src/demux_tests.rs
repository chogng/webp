use super::*;

fn limits() -> ContainerLimits {
    ContainerLimits::default()
}

fn riff(chunks: &[(FourCc, &[u8], Option<u8>)]) -> Vec<u8> {
    let mut body = b"WEBP".to_vec();
    for (fourcc, payload, padding) in chunks {
        body.extend_from_slice(fourcc);
        body.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        body.extend_from_slice(payload);
        if payload.len() % 2 == 1 {
            body.push(padding.unwrap_or(0));
        }
    }
    let mut output = b"RIFF".to_vec();
    output.extend_from_slice(&(body.len() as u32).to_le_bytes());
    output.extend_from_slice(&body);
    output
}

#[test]
fn every_riff_prefix_is_an_error_except_the_complete_file() {
    let valid = riff(&[(VP8, &[1, 2], None)]);
    for prefix in 0..valid.len() {
        assert!(
            parse(
                &valid[..prefix],
                CompatibilityProfile::SpecStrict,
                &limits()
            )
            .is_err(),
            "prefix {prefix}"
        );
    }
    assert!(parse(&valid, CompatibilityProfile::SpecStrict, &limits()).is_ok());
}

#[test]
fn odd_padding_is_checked_by_profile() {
    let valid = riff(&[(VP8, &[9], Some(0))]);
    assert_eq!(
        parse(&valid, CompatibilityProfile::SpecStrict, &limits())
            .unwrap()
            .chunks()[0]
            .padding,
        Some(0)
    );
    let non_zero = riff(&[(VP8, &[9], Some(8))]);
    assert!(parse(&non_zero, CompatibilityProfile::SpecStrict, &limits()).is_err());
    assert!(
        parse(
            &non_zero,
            CompatibilityProfile::LibwebpCompatible,
            &limits()
        )
        .is_ok()
    );
}

#[test]
fn compatible_profile_preserves_trailing_and_unknown_chunks() {
    let mut bytes = riff(&[(*b"zZZ!", &[7, 0, 8], Some(0)), (VP8, &[1, 2], None)]);
    bytes.extend_from_slice(&[0xaa, 0xbb]);
    assert!(parse(&bytes, CompatibilityProfile::SpecStrict, &limits()).is_err());
    let parsed = parse(&bytes, CompatibilityProfile::LibwebpCompatible, &limits()).unwrap();
    assert_eq!(parsed.trailing(), &[0xaa, 0xbb]);
    let unknown: Vec<_> = parsed.unknown_chunks().collect();
    assert_eq!(unknown.len(), 1);
    assert_eq!(unknown[0].fourcc, *b"zZZ!");
    assert_eq!(unknown[0].payload, &[7, 0, 8]);
}

#[test]
fn truncated_large_chunk_size_does_not_overrun() {
    let mut bytes = b"RIFF".to_vec();
    bytes.extend_from_slice(&12u32.to_le_bytes());
    bytes.extend_from_slice(b"WEBPVP8 ");
    bytes.extend_from_slice(&u32::MAX.to_le_bytes());
    assert_eq!(
        parse(&bytes, CompatibilityProfile::SpecStrict, &limits())
            .unwrap_err()
            .kind(),
        ContainerErrorKind::UnexpectedEof
    );
}

#[test]
fn vp8x_parses_canvas_and_extracts_raw_metadata() {
    let vp8x = [0b0010_1100, 0, 0, 0, 4, 0, 0, 2, 0, 0];
    let bytes = riff(&[
        (VP8X, &vp8x, None),
        (ICCP, &[1, 0], None),
        (EXIF, &[0xff], Some(0)),
        (XMP, b"x", Some(0)),
        (VP8, &[1, 2], None),
    ]);
    let parsed = parse(&bytes, CompatibilityProfile::SpecStrict, &limits()).unwrap();
    assert_eq!(parsed.vp8x().unwrap().canvas_width, 5);
    assert_eq!(parsed.vp8x().unwrap().canvas_height, 3);
    assert_eq!(
        parsed.metadata(),
        Metadata {
            iccp: Some(&[1, 0]),
            exif: Some(&[0xff]),
            xmp: Some(b"x")
        }
    );
}

#[test]
fn strict_rejects_vp8x_metadata_flag_mismatch() {
    let vp8x = [0; 10];
    let bytes = riff(&[
        (VP8X, &vp8x, None),
        (EXIF, &[1], Some(0)),
        (VP8, &[1, 2], None),
    ]);
    assert!(parse(&bytes, CompatibilityProfile::SpecStrict, &limits()).is_err());
    assert!(parse(&bytes, CompatibilityProfile::LibwebpCompatible, &limits()).is_ok());
}

#[test]
fn reserved_vp8x_bits_are_a_profile_decision() {
    let vp8x = [0b1000_0000, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    let bytes = riff(&[(VP8X, &vp8x, None), (VP8, &[1, 2], None)]);
    assert!(parse(&bytes, CompatibilityProfile::SpecStrict, &limits()).is_err());
    assert!(parse(&bytes, CompatibilityProfile::LibwebpCompatible, &limits()).is_ok());
}
