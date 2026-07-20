#![forbid(unsafe_code)]
//! Stable public API for the safe WebP implementation.
//!
//! M1 decodes static VP8L images to canonical RGBA8. M2 adds validated VP8
//! key-frame information; entropy decoding and pixel output are still pending.
//! Animation and incremental codec decoding remain outside this milestone.

pub use webp_core::{CompatibilityProfile, DecodeError, DecodeErrorKind, DecodeLimits};

/// Decode options that are stable before codec-specific tuning is introduced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodeOptions {
    pub limits: DecodeLimits,
    pub compatibility: CompatibilityProfile,
}

impl Default for DecodeOptions {
    fn default() -> Self {
        Self {
            limits: DecodeLimits::default(),
            compatibility: CompatibilityProfile::SpecStrict,
        }
    }
}

/// A decoded canonical straight-RGBA8 image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Image information available without allocating pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImageInfo {
    pub width: u32,
    pub height: u32,
    pub has_alpha: bool,
    pub is_animated: bool,
}

/// Raw metadata is intentionally unparsed and byte preserving.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Metadata {
    pub iccp: Option<Vec<u8>>,
    pub exif: Option<Vec<u8>>,
    pub xmp: Option<Vec<u8>>,
}

/// Progress from an incremental input state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Progress {
    NeedMoreData,
    Complete,
}

/// Buffers bounded incremental input. Codec-level incremental states will
/// replace this M0 buffer once VP8L and VP8 parsers exist.
#[derive(Debug, Clone)]
pub struct IncrementalDecoder {
    options: DecodeOptions,
    bytes: Vec<u8>,
    terminal: bool,
}

impl IncrementalDecoder {
    #[must_use]
    pub fn new(options: DecodeOptions) -> Self {
        Self {
            options,
            bytes: Vec::new(),
            terminal: false,
        }
    }

    pub fn push(&mut self, bytes: &[u8]) -> Result<Progress, DecodeError> {
        if self.terminal {
            return Err(DecodeError::at(
                DecodeErrorKind::InvalidParameter,
                self.bytes.len(),
                "push after finish",
            ));
        }
        let total = self.bytes.len().checked_add(bytes.len()).ok_or_else(|| {
            DecodeError::at(
                DecodeErrorKind::LimitExceeded,
                self.bytes.len(),
                "incremental input size overflow",
            )
        })?;
        if total > self.options.limits.max_input_bytes {
            return Err(DecodeError::at(
                DecodeErrorKind::LimitExceeded,
                total,
                "incremental input exceeds max_input_bytes",
            ));
        }
        self.bytes.try_reserve(bytes.len()).map_err(|_| {
            DecodeError::at(
                DecodeErrorKind::AllocationFailed,
                self.bytes.len(),
                "cannot reserve incremental input",
            )
        })?;
        self.bytes.extend_from_slice(bytes);
        Ok(Progress::NeedMoreData)
    }

    pub fn finish(mut self) -> Result<Image, DecodeError> {
        self.terminal = true;
        decode(&self.bytes, &self.options)
    }
}

/// Decodes a supported static WebP image to straight RGBA8.
///
/// M1 supports static VP8L images, including transforms, color cache,
/// meta-Huffman groups, and backward references. M2 currently validates VP8
/// headers but does not expose incomplete VP8 pixel output.
///
/// # Errors
///
/// Returns container-validation, codec, resource-limit, or unsupported-feature
/// errors. The function never substitutes an incomplete decode result.
pub fn decode(data: &[u8], options: &DecodeOptions) -> Result<Image, DecodeError> {
    let container = webp_container::parse(data, options.compatibility, &options.limits)?;
    if let Some(chunk) = container
        .chunks()
        .iter()
        .find(|chunk| chunk.fourcc == webp_container::VP8L)
    {
        let decoded = webp_vp8l_literal::decode_vp8l(chunk.payload, &options.limits)?;
        if let Some(vp8x) = container.vp8x()
            && (vp8x.canvas_width != decoded.header.width
                || vp8x.canvas_height != decoded.header.height)
        {
            return Err(DecodeError::at(
                DecodeErrorKind::InvalidContainer,
                chunk.offset,
                "VP8X canvas does not match VP8L dimensions",
            ));
        }
        return Ok(Image {
            width: decoded.header.width,
            height: decoded.header.height,
            rgba: decoded.rgba,
        });
    }
    if let Some(chunk) = container
        .chunks()
        .iter()
        .find(|chunk| chunk.fourcc == webp_container::VP8)
    {
        let canvas = container
            .vp8x()
            .map(|header| (header.canvas_width, header.canvas_height));
        webp_vp8::parse_riff_payload(chunk.payload, canvas, &options.limits)?;
        return Err(DecodeError::at(
            DecodeErrorKind::UnsupportedFeature,
            chunk.offset,
            "VP8 pixel decoding is not implemented yet",
        ));
    }
    Err(DecodeError::at(
        DecodeErrorKind::UnsupportedFeature,
        0,
        "this codec is not implemented by the M1 decoder",
    ))
}

/// Reads dimensions without pixel allocation.
///
/// VP8L and VP8 dimensions come from their fixed bitstream headers and must
/// agree with a present `VP8X` canvas.
///
/// # Errors
///
/// Returns the container or codec-header failure.
pub fn read_info(data: &[u8], limits: &DecodeLimits) -> Result<ImageInfo, DecodeError> {
    let container = webp_container::parse(data, CompatibilityProfile::SpecStrict, limits)?;
    if let Some(chunk) = container
        .chunks()
        .iter()
        .find(|chunk| chunk.fourcc == webp_container::VP8L)
    {
        let canvas = container
            .vp8x()
            .map(|header| (header.canvas_width, header.canvas_height));
        let header = webp_vp8l::parse_riff_payload(chunk.payload, canvas, limits)?;
        return Ok(ImageInfo {
            width: header.width,
            height: header.height,
            // This header field is an encoding hint. It may report false for
            // alpha-bearing pixels, but never changes decoded pixel recovery.
            has_alpha: header.alpha_is_used,
            is_animated: container
                .vp8x()
                .is_some_and(|header| header.flags.animation()),
        });
    }
    if let Some(chunk) = container
        .chunks()
        .iter()
        .find(|chunk| chunk.fourcc == webp_container::VP8)
    {
        let canvas = container
            .vp8x()
            .map(|header| (header.canvas_width, header.canvas_height));
        let header = webp_vp8::parse_riff_payload(chunk.payload, canvas, limits)?;
        return Ok(ImageInfo {
            width: header.width,
            height: header.height,
            has_alpha: container.vp8x().is_some_and(|vp8x| vp8x.flags.alpha()),
            is_animated: container.vp8x().is_some_and(|vp8x| vp8x.flags.animation()),
        });
    }
    let vp8x = container.vp8x().ok_or_else(|| {
        DecodeError::at(
            DecodeErrorKind::UnsupportedFeature,
            0,
            "M1 read_info requires VP8L or a VP8X header",
        )
    })?;
    Ok(ImageInfo {
        width: vp8x.canvas_width,
        height: vp8x.canvas_height,
        has_alpha: vp8x.flags.alpha(),
        is_animated: vp8x.flags.animation(),
    })
}

/// Extracts raw metadata after validating only the container and its limits.
pub fn read_metadata(data: &[u8], limits: &DecodeLimits) -> Result<Metadata, DecodeError> {
    let metadata =
        webp_container::parse(data, CompatibilityProfile::SpecStrict, limits)?.metadata();
    Ok(Metadata {
        iccp: metadata.iccp.map(ToOwned::to_owned),
        exif: metadata.exif.map(ToOwned::to_owned),
        xmp: metadata.xmp.map(ToOwned::to_owned),
    })
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
    };

    use super::*;

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

    fn fixture(relative_path: impl AsRef<Path>) -> Vec<u8> {
        let path = test_data_root().join(relative_path);
        fs::read(&path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
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

    fn assert_rejected(name: &str, bytes: &[u8]) {
        assert!(
            decode(bytes, &DecodeOptions::default()).is_err(),
            "{name}: one-shot decode must reject"
        );
        assert!(
            read_info(bytes, &DecodeLimits::default()).is_err(),
            "{name}: read_info must reject"
        );
        let mut incremental = IncrementalDecoder::new(DecodeOptions::default());
        incremental
            .push(bytes)
            .expect("fixture must fit the default input limit");
        assert!(
            incremental.finish().is_err(),
            "{name}: incremental finish must reject"
        );
    }

    #[test]
    fn direct_fixtures_exercise_public_decode_entrypoints() {
        let empty = fixture("fixtures/smoke/empty-input.webp");
        assert_rejected("empty-input.webp", &empty);

        for path in generated_fixtures() {
            let name = path.file_name().and_then(|name| name.to_str()).unwrap();
            if metadata_case(name).is_none() {
                let bytes = fs::read(&path)
                    .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
                assert_rejected(name, &bytes);
            }
        }
    }

    #[test]
    fn direct_metadata_fixtures_preserve_their_declared_payloads() {
        for path in generated_fixtures() {
            let name = path.file_name().and_then(|name| name.to_str()).unwrap();
            let Some((mask, length)) = metadata_case(name) else {
                continue;
            };
            let payload = (0..length)
                .map(|index| (index as u8).wrapping_add(mask))
                .collect::<Vec<_>>();
            let bytes =
                fs::read(&path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
            let metadata = read_metadata(&bytes, &DecodeLimits::default())
                .unwrap_or_else(|error| panic!("{name}: read_metadata failed: {error}"));
            for (label, actual, present) in [
                ("ICCP", metadata.iccp, mask & 1 != 0),
                ("EXIF", metadata.exif, mask & 2 != 0),
                ("XMP", metadata.xmp, mask & 4 != 0),
            ] {
                assert_eq!(
                    actual.as_deref(),
                    present.then_some(payload.as_slice()),
                    "{name}: {label} payload"
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
    fn read_info_accepts_a_simple_vp8l_header_without_pixel_allocation() {
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
    fn read_info_accepts_an_unextended_vp8_key_frame_without_pixel_allocation() {
        let payload = [0xf0, 0x00, 0x00, 0x9d, 0x01, 0x2a, 0x03, 0x00, 0x05, 0x00];
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
            DecodeErrorKind::UnsupportedFeature,
        );
    }

    #[test]
    fn decode_returns_rgba_for_the_supported_literal_only_vp8l_subset() {
        // A 1x1 lossless stream with no transforms/cache/meta groups and five
        // one-symbol Huffman codes. Pixel channels are RGBA = 12,34,56,78.
        use webp_core::BitWriter;

        let mut writer = BitWriter::new();
        writer.write_bits(0x2f, 8).unwrap();
        writer.write_bits(0, 14).unwrap();
        writer.write_bits(0, 14).unwrap();
        writer.write_bits(1, 1).unwrap();
        writer.write_bits(0, 3).unwrap();
        writer.write_bits(0, 3).unwrap(); // transform/cache/meta flags
        for channel in [0x34_u8, 0x12, 0x56, 0x78, 0] {
            writer.write_bits(1, 1).unwrap(); // simple code
            writer.write_bits(0, 1).unwrap(); // one symbol
            writer.write_bits(1, 1).unwrap(); // 8-bit symbol id
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
}
