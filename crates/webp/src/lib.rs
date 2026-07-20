#![forbid(unsafe_code)]
//! Stable public API for the safe WebP implementation.
//!
//! M1 validates VP8L headers and exposes container information. A deliberately
//! small no-transform, no-cache literal-only VP8L subset can already decode to
//! canonical RGBA8; all remaining codec features fail explicitly.

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

/// Pixel image placeholder for the upcoming canonical straight-RGBA8 decoder.
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

/// Decodes the currently supported WebP subset to straight RGBA8.
///
/// M1 supports static VP8L images with no transforms, color cache, meta-Huffman
/// image, or backward references. Other valid WebP features return
/// `UnsupportedFeature` rather than producing partial pixel output.
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
        let decoded = webp_vp8l_literal::decode_literal_only(chunk.payload, &options.limits)?;
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
    Err(DecodeError::at(
        DecodeErrorKind::UnsupportedFeature,
        0,
        "this codec is not implemented by the M1 decoder",
    ))
}

/// Reads dimensions without pixel allocation.
///
/// VP8L dimensions come from its fixed bitstream header and must agree with a
/// present `VP8X` canvas. For VP8, M1 can only report dimensions from `VP8X`.
///
/// # Errors
///
/// Returns the container or VP8L-header failure, or `UnsupportedFeature` when
/// an unextended VP8 frame requires the not-yet-implemented VP8 header parser.
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
    use super::*;
    use webp_testkit::{FixtureClass, FixtureRunner};

    #[test]
    fn smoke_manifests_exercise_each_public_decode_entrypoint() {
        let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests");
        let summary = FixtureRunner::new(root)
            .run_all(|fixture, bytes| {
                match fixture.class {
                    FixtureClass::MustReject => {
                        assert!(
                            decode(bytes, &DecodeOptions::default()).is_err(),
                            "{}: one-shot decode must reject",
                            fixture.id
                        );
                        assert!(
                            read_info(bytes, &DecodeLimits::default()).is_err(),
                            "{}: read_info must reject",
                            fixture.id
                        );
                        let mut incremental = IncrementalDecoder::new(DecodeOptions::default());
                        incremental
                            .push(bytes)
                            .expect("input must fit default limit");
                        assert!(
                            incremental.finish().is_err(),
                            "{}: incremental finish must reject",
                            fixture.id
                        );
                    }
                    FixtureClass::MustAccept => {
                        assert!(
                            decode(bytes, &DecodeOptions::default()).is_ok(),
                            "{} must accept",
                            fixture.id
                        );
                    }
                    FixtureClass::CompatAccept | FixtureClass::ImplementationDefined => {}
                }
                Ok::<_, String>(())
            })
            .expect("all smoke fixtures must run");
        assert!(summary.fixtures > 0, "smoke corpus must not be empty");
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
