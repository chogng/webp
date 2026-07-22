//! Conversion at the independent container boundary.

use crate::CompatibilityProfile;
use crate::DecodeError;
use crate::DecodeErrorKind;
use crate::DecodeLimits;

pub(crate) fn parse<'a>(
    data: &'a [u8],
    profile: CompatibilityProfile,
    limits: &DecodeLimits,
) -> Result<webp_container::Container<'a>, DecodeError> {
    webp_container::parse(data, map_profile(profile), &map_limits(limits)).map_err(map_parse_error)
}

fn map_profile(profile: CompatibilityProfile) -> webp_container::CompatibilityProfile {
    match profile {
        CompatibilityProfile::SpecStrict => webp_container::CompatibilityProfile::SpecStrict,
        CompatibilityProfile::LibwebpCompatible => {
            webp_container::CompatibilityProfile::LibwebpCompatible
        }
    }
}

fn map_limits(limits: &DecodeLimits) -> webp_container::ContainerLimits {
    webp_container::ContainerLimits {
        max_input_bytes: limits.max_input_bytes,
        max_width: limits.max_width,
        max_height: limits.max_height,
        max_pixels: limits.max_pixels,
        max_frames: limits.max_frames,
        max_total_frame_pixels: limits.max_total_frame_pixels,
        max_metadata_bytes: limits.max_metadata_bytes,
        max_chunks: webp_container::ContainerLimits::default().max_chunks,
    }
}

fn map_parse_error(error: webp_container::ContainerError) -> DecodeError {
    let kind = match error.kind() {
        webp_container::ContainerErrorKind::UnexpectedEof => DecodeErrorKind::UnexpectedEof,
        webp_container::ContainerErrorKind::LimitExceeded => DecodeErrorKind::LimitExceeded,
        webp_container::ContainerErrorKind::AllocationFailed => DecodeErrorKind::AllocationFailed,
        webp_container::ContainerErrorKind::InvalidContainer
        | webp_container::ContainerErrorKind::SizeOverflow
        | webp_container::ContainerErrorKind::InvalidDimensions
        | webp_container::ContainerErrorKind::InvalidAnimation => DecodeErrorKind::InvalidContainer,
    };
    DecodeError::new(kind, error.offset(), error.context())
}
