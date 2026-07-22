//! Public data types shared by the decode entry points.

use webp_core::CompatibilityProfile;
use webp_core::DecodeLimits;

/// Stable reason a WebP encoding operation failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncodeError {
    /// Width or height is zero or exceeds VP8L's 14-bit dimension field.
    InvalidDimensions,
    /// The RGBA input is not exactly `width * height * 4` bytes.
    InvalidRgbaLength,
    /// Image or output byte-size arithmetic overflowed the host address space.
    SizeOverflow,
    /// Reserving output storage failed.
    AllocationFailed,
    /// Animation frame geometry, timing, or composition flags are invalid.
    InvalidAnimation,
    /// The requested VP8 quality is outside the supported 0 through 100 range.
    InvalidQuality,
    /// The selected lossy VP8 profile is not implemented by the current encoder.
    UnsupportedLossyProfile,
}

impl EncodeError {
    pub(crate) const fn invalid_dimensions() -> Self {
        Self::InvalidDimensions
    }
    pub(crate) const fn invalid_rgba_length() -> Self {
        Self::InvalidRgbaLength
    }
    pub(crate) const fn input_size_overflow() -> Self {
        Self::SizeOverflow
    }
    pub(crate) const fn output_size_overflow() -> Self {
        Self::SizeOverflow
    }
    pub(crate) const fn allocation_failed() -> Self {
        Self::AllocationFailed
    }
    pub(crate) const fn invalid_animation() -> Self {
        Self::InvalidAnimation
    }
    pub(crate) const fn invalid_quality() -> Self {
        Self::InvalidQuality
    }
    pub(crate) const fn unsupported_lossy_profile() -> Self {
        Self::UnsupportedLossyProfile
    }
}

impl core::fmt::Display for EncodeError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidDimensions => formatter.write_str("invalid VP8L image dimensions"),
            Self::InvalidRgbaLength => {
                formatter.write_str("RGBA input length does not match dimensions")
            }
            Self::SizeOverflow => formatter.write_str("WebP output size overflow"),
            Self::AllocationFailed => formatter.write_str("WebP output allocation failed"),
            Self::InvalidAnimation => formatter.write_str("invalid WebP animation frame"),
            Self::InvalidQuality => formatter.write_str("VP8 quality must be in 0 through 100"),
            Self::UnsupportedLossyProfile => {
                formatter.write_str("the requested lossy VP8 profile is not implemented")
            }
        }
    }
}

impl std::error::Error for EncodeError {}

/// Explicit configuration for the bounded static lossy VP8 encoder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LossyEncodeOptions {
    /// VP8 quantization quality on a 0 (smallest output) through 100 scale.
    pub quality: u8,
}

impl Default for LossyEncodeOptions {
    fn default() -> Self {
        Self { quality: 75 }
    }
}

/// Stable size/decoding-latency tradeoffs for static lossless encoding.
///
/// Every profile emits an ordinary VP8L bitstream. The fast-decode profiles
/// use coarse spatial Huffman groups and are never selected implicitly. Their
/// names describe the tradeoff relative to each profile's fast-no-cache
/// single-group stream; they can be larger than [`Self::Default`] and are
/// currently substantially more expensive to encode because both complete
/// candidate files are serialized before selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum LosslessEncodeProfile {
    /// Preserve the encoder's established output and behavior.
    #[default]
    Default,
    /// Prefer the more compact validated fast-decode tradeoff.
    ///
    /// This profile uses 128-pixel spatial blocks and at most 64 entropy
    /// groups, then falls back byte-for-byte to its single-group stream unless
    /// the complete coarse WebP file is strictly smaller.
    FastDecodeCompact,
    /// Prefer fewer entropy groups for lower decoding latency.
    ///
    /// This profile uses 256-pixel spatial blocks and at most 16 entropy
    /// groups, then falls back byte-for-byte to its single-group stream unless
    /// the complete coarse WebP file is strictly smaller.
    FastDecodeLowLatency,
}

/// Options for static lossless WebP encoding.
///
/// This type is non-exhaustive so future encoder controls can be added without
/// making source-compatible callers construct new fields. Start from
/// [`Self::default`] and update the desired fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct LosslessEncodeOptions {
    /// Selects the encoder's lossless size/decoding-latency tradeoff.
    pub profile: LosslessEncodeProfile,
}

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// A fully composed frame in display order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnimationFrame {
    /// The frame's declared display time in milliseconds.
    pub duration_ms: u32,
    /// Complete canvas contents after blending and disposal, in straight RGBA8.
    pub rgba: Vec<u8>,
}

/// A decoded WebP animation with display-ready canvas frames.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Animation {
    pub width: u32,
    pub height: u32,
    /// `0` represents infinitely many loops.
    pub loop_count: u16,
    pub frames: Vec<AnimationFrame>,
}

/// Global settings for a lossless WebP animation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AnimationEncodeOptions {
    /// Canvas fill color used by dispose-to-background frames, in straight RGBA8.
    pub background_rgba: [u8; 4],
    /// Number of animation loops; `0` represents infinitely many loops.
    pub loop_count: u16,
}

/// One rectangle of a lossless WebP animation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnimationEncodeFrame<'a> {
    /// Even horizontal canvas offset in pixels.
    pub x: u32,
    /// Even vertical canvas offset in pixels.
    pub y: u32,
    pub width: u32,
    pub height: u32,
    /// Display duration in milliseconds, representable as an unsigned 24-bit value.
    pub duration_ms: u32,
    /// Straight/unpremultiplied RGBA8 pixels in row-major frame-rectangle order.
    pub rgba: &'a [u8],
    /// Restore this rectangle to the configured background after display.
    pub dispose_to_background: bool,
    /// Blend this frame over the current canvas; `false` overwrites the rectangle.
    pub blend: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImageInfo {
    pub width: u32,
    pub height: u32,
    pub has_alpha: bool,
    pub is_animated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Metadata {
    pub iccp: Option<Vec<u8>>,
    pub exif: Option<Vec<u8>>,
    pub xmp: Option<Vec<u8>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Progress {
    NeedMoreData,
    Complete,
}
