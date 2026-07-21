use std::fs;
use std::path::PathBuf;

use webp::DecodeErrorKind;
use webp::DecodeLimits;
use webp::DecodeOptions;
use webp::Image;
use webp::ImageInfo;
use webp::IncrementalDecoder;
use webp::Progress;
use webp::decode;
use webp::read_info;
use webp::read_metadata;

fn test_data_root() -> PathBuf {
    if let Some(runfiles) = std::env::var_os("TEST_SRCDIR") {
        let bazel_root = PathBuf::from(runfiles).join("_main/tests");
        if bazel_root.is_dir() {
            return bazel_root;
        }
    }
    let cargo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests");
    if cargo_root.is_dir() {
        return cargo_root;
    }
    panic!("test fixtures are unavailable")
}

fn generated_fixtures() -> Vec<PathBuf> {
    let root = test_data_root().join("fixtures/generated");
    let mut fixtures = fs::read_dir(&root)
        .unwrap_or_else(|error| panic!("read {}: {error}", root.display()))
        .map(|entry| entry.expect("read generated fixture entry").path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "webp")
        })
        .collect::<Vec<_>>();
    fixtures.sort();
    fixtures
}

fn metadata_case(name: &str) -> Option<(u8, usize)> {
    let stem = name.strip_suffix(".webp")?;
    let mut parts = stem.split('-');
    if parts.next()? != "metadata" {
        return None;
    }
    let mask = u8::from_str_radix(parts.next()?, 16).ok()?;
    let length = parts.next()?.parse().ok()?;
    matches!(parts.next()?, "before" | "after").then_some((mask, length))
}

#[test]
fn malformed_fixtures_are_rejected_by_public_entrypoints() {
    let empty = fs::read(test_data_root().join("fixtures/smoke/empty-input.webp")).unwrap();
    assert!(decode(&empty, &DecodeOptions::default()).is_err());
    assert!(read_info(&empty, &DecodeLimits::default()).is_err());
    for path in generated_fixtures() {
        let name = path.file_name().and_then(|name| name.to_str()).unwrap();
        if metadata_case(name).is_some() {
            continue;
        }
        let bytes =
            fs::read(&path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
        assert!(
            decode(&bytes, &DecodeOptions::default()).is_err(),
            "{name}: decode"
        );
        assert!(
            read_info(&bytes, &DecodeLimits::default()).is_err(),
            "{name}: read_info"
        );
        let mut incremental = IncrementalDecoder::new(DecodeOptions::default());
        incremental.push(&bytes).unwrap();
        assert!(incremental.finish().is_err(), "{name}: incremental");
    }
}

#[test]
fn metadata_fixtures_preserve_declared_payloads() {
    for path in generated_fixtures() {
        let name = path.file_name().and_then(|name| name.to_str()).unwrap();
        let Some((mask, length)) = metadata_case(name) else {
            continue;
        };
        let expected = (0..length)
            .map(|index| (index as u8).wrapping_add(mask))
            .collect::<Vec<_>>();
        let bytes =
            fs::read(&path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
        let metadata = read_metadata(&bytes, &DecodeLimits::default()).unwrap();
        for (actual, present) in [
            (metadata.iccp, mask & 1 != 0),
            (metadata.exif, mask & 2 != 0),
            (metadata.xmp, mask & 4 != 0),
        ] {
            assert_eq!(
                actual.as_deref(),
                present.then_some(expected.as_slice()),
                "{name}"
            );
        }
    }
}

#[test]
fn incremental_size_limit_is_enforced_before_append() {
    let options = DecodeOptions {
        limits: DecodeLimits {
            max_input_bytes: 1,
            ..DecodeLimits::default()
        },
        ..DecodeOptions::default()
    };
    let mut decoder = IncrementalDecoder::new(options);
    assert_eq!(decoder.push(&[1]).unwrap(), Progress::NeedMoreData);
    assert_eq!(
        decoder.push(&[2]).unwrap_err().kind(),
        DecodeErrorKind::LimitExceeded
    );
}

#[test]
fn read_info_accepts_small_vp8l_headers() {
    let mut bytes = b"RIFF".to_vec();
    bytes.extend_from_slice(&18_u32.to_le_bytes());
    bytes.extend_from_slice(b"WEBPVP8L");
    bytes.extend_from_slice(&5_u32.to_le_bytes());
    bytes.extend_from_slice(&[0x2f, 0, 0, 0, 0]);
    bytes.push(0);
    assert_eq!(
        read_info(&bytes, &DecodeLimits::default()).unwrap(),
        ImageInfo {
            width: 1,
            height: 1,
            has_alpha: false,
            is_animated: false,
        }
    );
}

#[test]
fn read_info_accepts_small_vp8_headers_without_decoding_pixels() {
    let payload = [0x10, 0x00, 0x00, 0x9d, 0x01, 0x2a, 0x03, 0x00, 0x05, 0x00];
    let mut bytes = b"RIFF".to_vec();
    bytes.extend_from_slice(&22_u32.to_le_bytes());
    bytes.extend_from_slice(b"WEBPVP8 ");
    bytes.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&payload);
    assert_eq!(
        read_info(&bytes, &DecodeLimits::default()).unwrap(),
        ImageInfo {
            width: 3,
            height: 5,
            has_alpha: false,
            is_animated: false,
        }
    );
    assert_eq!(
        decode(&bytes, &DecodeOptions::default())
            .unwrap_err()
            .kind(),
        DecodeErrorKind::UnexpectedEof,
    );
}

#[test]
fn decode_returns_rgba_for_a_literal_vp8l_pixel() {
    use webp_core::BitWriter;

    let mut writer = BitWriter::new();
    writer.write_bits(0x2f, 8).unwrap();
    writer.write_bits(0, 14).unwrap();
    writer.write_bits(0, 14).unwrap();
    writer.write_bits(1, 1).unwrap();
    writer.write_bits(0, 3).unwrap();
    writer.write_bits(0, 3).unwrap();
    for channel in [0x34_u8, 0x12, 0x56, 0x78, 0] {
        writer.write_bits(1, 1).unwrap();
        writer.write_bits(0, 1).unwrap();
        writer.write_bits(1, 1).unwrap();
        writer.write_bits(u32::from(channel), 8).unwrap();
    }
    let payload = writer.into_bytes();
    let mut body = b"WEBPVP8L".to_vec();
    body.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    body.extend_from_slice(&payload);
    if payload.len() % 2 == 1 {
        body.push(0);
    }
    let mut bytes = b"RIFF".to_vec();
    bytes.extend_from_slice(&(body.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&body);
    assert_eq!(
        decode(&bytes, &DecodeOptions::default()).unwrap(),
        Image {
            width: 1,
            height: 1,
            rgba: vec![0x12, 0x34, 0x56, 0x78],
        }
    );
}
