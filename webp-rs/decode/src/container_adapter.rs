//! Conversion at the independent container boundary.

use crate::CompatibilityProfile;
use crate::DecodeError;
use crate::DecodeErrorKind;
use crate::DecodeLimits;

pub(crate) fn parse<'a>(
    data: &'a [u8],
    profile: CompatibilityProfile,
    limits: &DecodeLimits,
) -> Result<webp_demux::Container<'a>, DecodeError> {
    webp_demux::parse(data, map_profile(profile), &map_limits(limits)).map_err(map_parse_error)
}

fn map_profile(profile: CompatibilityProfile) -> webp_demux::CompatibilityProfile {
    match profile {
        CompatibilityProfile::SpecStrict => webp_demux::CompatibilityProfile::SpecStrict,
        CompatibilityProfile::LibwebpCompatible => {
            webp_demux::CompatibilityProfile::LibwebpCompatible
        }
    }
}

fn map_limits(limits: &DecodeLimits) -> webp_demux::ContainerLimits {
    webp_demux::ContainerLimits {
        max_input_bytes: limits.max_input_bytes,
        max_width: limits.max_width,
        max_height: limits.max_height,
        max_pixels: limits.max_pixels,
        max_frames: limits.max_frames,
        max_total_frame_pixels: limits.max_total_frame_pixels,
        max_metadata_bytes: limits.max_metadata_bytes,
        max_chunks: webp_demux::ContainerLimits::default().max_chunks,
    }
}

fn map_parse_error(error: webp_demux::ContainerError) -> DecodeError {
    let kind = match error.kind() {
        webp_demux::ContainerErrorKind::UnexpectedEof => DecodeErrorKind::UnexpectedEof,
        webp_demux::ContainerErrorKind::LimitExceeded => DecodeErrorKind::LimitExceeded,
        webp_demux::ContainerErrorKind::AllocationFailed => DecodeErrorKind::AllocationFailed,
        webp_demux::ContainerErrorKind::InvalidContainer
        | webp_demux::ContainerErrorKind::SizeOverflow
        | webp_demux::ContainerErrorKind::InvalidDimensions
        | webp_demux::ContainerErrorKind::InvalidAnimation => DecodeErrorKind::InvalidContainer,
    };
    DecodeError::new(kind, error.offset(), error.context())
}
