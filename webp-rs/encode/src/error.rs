//! Stable errors for encoding operations.

use core::fmt;

/// Stable reason a WebP encoding operation failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncodeError {
    InvalidDimensions,
    InvalidRgbaLength,
    SizeOverflow,
    AllocationFailed,
    InvalidAnimation,
    InvalidQuality,
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

impl fmt::Display for EncodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
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
