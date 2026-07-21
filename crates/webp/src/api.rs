//! Public data types shared by the decode entry points.

use webp_core::{CompatibilityProfile, DecodeLimits};

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
        }
    }
}

impl std::error::Error for EncodeError {}

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
