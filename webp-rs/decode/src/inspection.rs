//! Header-only image information and metadata extraction.

use crate::CompatibilityProfile;
use crate::DecodeError;
use crate::DecodeErrorKind;
use crate::DecodeLimits;
use crate::ImageInfo;
use crate::Metadata;

pub fn read_info(data: &[u8], limits: &DecodeLimits) -> Result<ImageInfo, DecodeError> {
    let container =
        crate::container_adapter::parse(data, CompatibilityProfile::SpecStrict, limits)?;
    if let Some(chunk) = container
        .chunks()
        .iter()
        .find(|chunk| chunk.fourcc == webp_container::VP8L)
    {
        let canvas = container
            .vp8x()
            .map(|header| (header.canvas_width, header.canvas_height));
        let header = crate::vp8l::header::parse_riff_payload(chunk.payload, canvas, limits)?;
        return Ok(ImageInfo {
            width: header.width,
            height: header.height,
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
        let header = crate::vp8::parse_riff_payload(chunk.payload, canvas, limits)?;
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

pub fn read_metadata(data: &[u8], limits: &DecodeLimits) -> Result<Metadata, DecodeError> {
    let metadata =
        crate::container_adapter::parse(data, CompatibilityProfile::SpecStrict, limits)?.metadata();
    Ok(Metadata {
        iccp: metadata.iccp.map(ToOwned::to_owned),
        exif: metadata.exif.map(ToOwned::to_owned),
        xmp: metadata.xmp.map(ToOwned::to_owned),
    })
}
