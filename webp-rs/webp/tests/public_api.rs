#![cfg(feature = "decode")]

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
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

const FIXTURE_MANIFEST_HEADER: &str = "webp-fixture-manifest-v1";
const CURRENT_PREFIX: &str = "CURRENT-";

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
    let (sequence, digest, marker) = current_fixture_marker(&root);
    let marker_contents = fs::read_to_string(&marker)
        .unwrap_or_else(|error| panic!("read {}: {error}", marker.display()));
    assert_eq!(
        marker_contents,
        format!("{digest}\n"),
        "fixture marker {sequence} is malformed"
    );

    let generation = root.join("sets").join(digest);
    let manifest_path = generation.join("MANIFEST.sha256");
    let manifest = fs::read_to_string(&manifest_path)
        .unwrap_or_else(|error| panic!("read {}: {error}", manifest_path.display()));
    let mut lines = manifest.lines();
    assert_eq!(
        lines.next(),
        Some(FIXTURE_MANIFEST_HEADER),
        "fixture manifest has an unsupported schema"
    );

    let mut names = BTreeSet::new();
    let mut fixtures = Vec::new();
    for line in lines {
        let mut fields = line.splitn(3, ' ');
        let hash = fields.next().unwrap_or_default();
        let size = fields
            .next()
            .and_then(|size| size.parse::<u64>().ok())
            .expect("fixture manifest contains an invalid size");
        let name = fields
            .next()
            .filter(|name| safe_fixture_name(name))
            .expect("fixture manifest contains an unsafe file name");
        assert!(
            hash.len() == 64 && hash.bytes().all(|byte| byte.is_ascii_hexdigit()),
            "fixture manifest contains an invalid SHA-256"
        );
        assert!(names.insert(name.to_owned()), "duplicate fixture {name}");
        let path = generation.join(name);
        let metadata = fs::metadata(&path)
            .unwrap_or_else(|error| panic!("inspect {}: {error}", path.display()));
        assert!(metadata.is_file(), "{} is not a file", path.display());
        assert_eq!(metadata.len(), size, "{} size mismatch", path.display());
        fixtures.push(path);
    }

    let actual_names = fs::read_dir(&generation)
        .unwrap_or_else(|error| panic!("read {}: {error}", generation.display()))
        .map(|entry| entry.expect("read generated fixture entry").file_name())
        .filter_map(|name| name.into_string().ok())
        .filter(|name| name.ends_with(".webp"))
        .collect::<BTreeSet<_>>();
    assert_eq!(
        actual_names, names,
        "fixture generation has missing or unexpected WebP files"
    );
    assert!(!fixtures.is_empty(), "fixture manifest is empty");
    fixtures
}

fn current_fixture_marker(root: &Path) -> (u64, String, PathBuf) {
    fs::read_dir(root)
        .unwrap_or_else(|error| {
            panic!(
                "fixture cache is unavailable at {}: {error}; run `cargo run -p xtask -- fixtures ensure`",
                root.display()
            )
        })
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let name = entry.file_name().into_string().ok()?;
            let rest = name.strip_prefix(CURRENT_PREFIX)?;
            let (sequence, digest) = rest.split_once('-')?;
            if sequence.len() != 20
                || digest.len() != 64
                || !sequence.bytes().all(|byte| byte.is_ascii_digit())
                || !digest.bytes().all(|byte| byte.is_ascii_hexdigit())
                || !entry.file_type().ok()?.is_file()
            {
                return None;
            }
            Some((sequence.parse::<u64>().ok()?, digest.to_owned(), entry.path()))
        })
        .max_by_key(|(sequence, _, _)| *sequence)
        .unwrap_or_else(|| {
            panic!(
                "fixture cache has no committed generation; run `cargo run -p xtask -- fixtures ensure`"
            )
        })
}

fn safe_fixture_name(name: &str) -> bool {
    !name.is_empty()
        && name.ends_with(".webp")
        && !name.contains(['/', '\\', '\n', '\r', ' '])
        && name != "."
        && name != ".."
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
    assert!(decode(&[], &DecodeOptions::default()).is_err());
    assert!(read_info(&[], &DecodeLimits::default()).is_err());
    for path in generated_fixtures() {
        let name = path.file_name().and_then(|name| name.to_str()).unwrap();
        if metadata_case(name).is_some() {
            continue;
        }
        let bytes =
            fs::read(&path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
        let one_shot = decode(&bytes, &DecodeOptions::default()).unwrap_err();
        assert!(
            read_info(&bytes, &DecodeLimits::default()).is_err(),
            "{name}: read_info"
        );
        let mut incremental = IncrementalDecoder::new(DecodeOptions::default());
        let incremental_result = incremental
            .push(&bytes)
            .map(|_| incremental.finish())
            .unwrap_or_else(Err);
        assert_eq!(
            incremental_result.unwrap_err().kind(),
            one_shot.kind(),
            "{name}: incremental"
        );
    }
}

#[test]
fn malformed_fixtures_are_rejected_at_every_split() {
    for path in generated_fixtures() {
        let name = path.file_name().and_then(|name| name.to_str()).unwrap();
        if metadata_case(name).is_some() {
            continue;
        }
        let bytes = fs::read(&path).unwrap();
        for split in 0..=bytes.len() {
            let mut decoder = IncrementalDecoder::new(DecodeOptions::default());
            let result = decoder
                .push(&bytes[..split])
                .and_then(|progress| {
                    if progress == Progress::Complete {
                        Ok(progress)
                    } else {
                        decoder.push(&bytes[split..])
                    }
                })
                .map(|_| decoder.finish())
                .unwrap_or_else(Err);
            assert!(result.is_err(), "{name}: split={split}");
        }
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
    let mut writer = TestBitWriter::default();
    writer.write_bits(0x2f, 8);
    writer.write_bits(0, 14);
    writer.write_bits(0, 14);
    writer.write_bits(1, 1);
    writer.write_bits(0, 3);
    writer.write_bits(0, 3);
    for channel in [0x34_u8, 0x12, 0x56, 0x78, 0] {
        writer.write_bits(1, 1);
        writer.write_bits(0, 1);
        writer.write_bits(1, 1);
        writer.write_bits(u32::from(channel), 8);
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
    let expected = Image {
        width: 1,
        height: 1,
        rgba: vec![0x12, 0x34, 0x56, 0x78],
    };
    assert_eq!(decode(&bytes, &DecodeOptions::default()).unwrap(), expected);
    for split in 0..=bytes.len() {
        let mut incremental = IncrementalDecoder::new(DecodeOptions::default());
        let first = incremental.push(&bytes[..split]).unwrap();
        if first != Progress::Complete {
            assert_eq!(
                incremental.push(&bytes[split..]).unwrap(),
                Progress::Complete
            );
        }
        let view = incremental.decoded().unwrap();
        assert_eq!(view.decoded_rows, 1);
        assert_eq!(view.rgba, expected.rgba);
        assert_eq!(incremental.info().unwrap().width, 1);
        assert_eq!(
            incremental.push(&[]).unwrap_err().kind(),
            DecodeErrorKind::InvalidParameter
        );
        assert_eq!(incremental.finish().unwrap(), expected, "split={split}");
    }
}

#[derive(Default)]
struct TestBitWriter {
    bytes: Vec<u8>,
    current: u8,
    used: u8,
}

impl TestBitWriter {
    fn write_bits(&mut self, mut value: u32, count: u8) {
        for _ in 0..count {
            self.current |= ((value & 1) as u8) << self.used;
            self.used += 1;
            value >>= 1;
            if self.used == u8::BITS as u8 {
                self.bytes.push(self.current);
                self.current = 0;
                self.used = 0;
            }
        }
    }

    fn into_bytes(mut self) -> Vec<u8> {
        if self.used != 0 {
            self.bytes.push(self.current);
        }
        self.bytes
    }
}
