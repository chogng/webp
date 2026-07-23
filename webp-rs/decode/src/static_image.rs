//! Static-image result and decode orchestration.

use crate::DecodeError;
use crate::DecodeErrorKind;
use crate::DecodeOptions;

/// A decoded static WebP image in straight RGBA8.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Decodes a supported static WebP image to straight RGBA8.
///
/// M1 supports static VP8L images, including transforms, color cache,
/// meta-Huffman groups, and backward references. M2 supports VP8 key frames.
/// M3 supports their `ALPH` planes. With the `animation` feature, animated
/// containers use the separate animation decode API; incremental codec state
/// remains unavailable.
///
/// # Errors
///
/// Returns container-validation, codec, resource-limit, or unsupported-feature
/// errors. The function never substitutes an incomplete decode result.
pub fn decode(data: &[u8], options: &DecodeOptions) -> Result<Image, DecodeError> {
    let container = crate::container_adapter::parse(data, options.compatibility, &options.limits)?;
    if let Some(chunk) = container
        .chunks()
        .iter()
        .find(|chunk| chunk.fourcc == webp_container::VP8L)
    {
        let decoded = crate::vp8l::image_reader::decode_vp8l(chunk.payload, &options.limits)?;
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
        let alpha = container
            .chunks()
            .iter()
            .enumerate()
            .find(|(_, candidate)| candidate.fourcc == webp_container::ALPH);
        if container.vp8x().is_some_and(|header| header.flags.alpha()) && alpha.is_none() {
            return Err(DecodeError::at(
                DecodeErrorKind::InvalidContainer,
                chunk.offset,
                "VP8X declares alpha but has no ALPH chunk",
            ));
        }
        let header = crate::vp8::parse_riff_payload(chunk.payload, canvas, &options.limits)?;
        let yuv = crate::vp8::decode_intra_frame(chunk.payload, &header, &options.limits)?;
        let mut rgba = yuv.to_rgba(&options.limits)?;
        if let Some((alpha_index, alpha_chunk)) = alpha {
            let vp8_index = container
                .chunks()
                .iter()
                .position(|candidate| candidate.fourcc == webp_container::VP8)
                .expect("selected VP8 chunk has an index");
            if alpha_index > vp8_index {
                return Err(DecodeError::at(
                    DecodeErrorKind::InvalidContainer,
                    alpha_chunk.offset,
                    "ALPH chunk must precede its VP8 chunk",
                ));
            }
            let plane = crate::alpha::decode::decode(
                alpha_chunk.payload,
                header.width,
                header.height,
                options.compatibility,
                &options.limits,
            )?;
            for (pixel, alpha) in rgba.chunks_exact_mut(4).zip(plane) {
                pixel[3] = alpha;
            }
        }
        return Ok(Image {
            width: header.width,
            height: header.height,
            rgba,
        });
    }
    Err(DecodeError::at(
        DecodeErrorKind::UnsupportedFeature,
        0,
        "this codec is not implemented by the M1 decoder",
    ))
}
