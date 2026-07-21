//! Internal static-image decode dispatch.

use crate::Animation;
use crate::AnimationFrame;
use crate::DecodeOptions;
use crate::Image;
use webp_animation::AnimationCanvas;
use webp_animation::DecodedFrame;
use webp_container::FrameBitstream;
use webp_core::DecodeError;
use webp_core::DecodeErrorKind;
use webp_core::checked_image_bytes;

pub(crate) fn decode(data: &[u8], options: &DecodeOptions) -> Result<Image, DecodeError> {
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
        let header = webp_vp8::parse_riff_payload(chunk.payload, canvas, &options.limits)?;
        let yuv = webp_vp8::decode_intra_frame(chunk.payload, &header, &options.limits)?;
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
            let plane = webp_alpha::decode(
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

pub(crate) fn decode_animation(
    data: &[u8],
    options: &DecodeOptions,
) -> Result<Animation, DecodeError> {
    let container = webp_container::parse(data, options.compatibility, &options.limits)?;
    let animation = container.animation().ok_or_else(|| {
        DecodeError::at(
            DecodeErrorKind::UnsupportedFeature,
            0,
            "decode_animation requires an animated WebP container",
        )
    })?;
    let vp8x = container.vp8x().expect("animation requires VP8X");
    let canvas_bytes = checked_image_bytes(vp8x.canvas_width, vp8x.canvas_height, 4)?;
    let total_output_bytes = canvas_bytes
        .checked_mul(animation.frames().len())
        .ok_or_else(|| {
            DecodeError::new(
                DecodeErrorKind::LimitExceeded,
                None,
                "animation output size overflow",
            )
        })?;
    if total_output_bytes > options.limits.max_alloc_bytes {
        return Err(DecodeError::new(
            DecodeErrorKind::LimitExceeded,
            None,
            "animation output exceeds configured allocation limit",
        ));
    }

    let mut canvas = AnimationCanvas::new(
        vp8x.canvas_width,
        vp8x.canvas_height,
        animation.background_bgra,
        &options.limits,
    )?;
    let mut frames = Vec::new();
    frames
        .try_reserve_exact(animation.frames().len())
        .map_err(|_| allocation_failed("cannot reserve decoded animation frames"))?;
    for frame in animation.frames() {
        let rgba = decode_animation_frame(frame, options)?;
        canvas.compose(
            DecodedFrame {
                x: frame.x,
                y: frame.y,
                width: frame.width,
                height: frame.height,
                rgba: &rgba,
                blend: frame.blend,
                dispose_to_background: frame.dispose_to_background,
            },
            &options.limits,
        )?;
        frames.push(AnimationFrame {
            duration_ms: frame.duration_ms,
            rgba: clone_canvas(canvas.rgba())?,
        });
    }
    Ok(Animation {
        width: vp8x.canvas_width,
        height: vp8x.canvas_height,
        loop_count: animation.loop_count,
        frames,
    })
}

fn decode_animation_frame(
    frame: &webp_container::AnimationFrame<'_>,
    options: &DecodeOptions,
) -> Result<Vec<u8>, DecodeError> {
    let mut rgba = match frame.bitstream {
        FrameBitstream::Vp8l(payload) => {
            let decoded = webp_vp8l_literal::decode_vp8l(payload, &options.limits)?;
            if (decoded.header.width, decoded.header.height) != (frame.width, frame.height) {
                return Err(DecodeError::new(
                    DecodeErrorKind::InvalidContainer,
                    None,
                    "ANMF VP8L dimensions do not match its frame rectangle",
                ));
            }
            decoded.rgba
        }
        FrameBitstream::Vp8(payload) => {
            let header = webp_vp8::parse_riff_payload(
                payload,
                Some((frame.width, frame.height)),
                &options.limits,
            )?;
            webp_vp8::decode_intra_frame(payload, &header, &options.limits)?
                .to_rgba(&options.limits)?
        }
    };
    if let Some(alpha) = frame.alpha {
        let plane = webp_alpha::decode(
            alpha,
            frame.width,
            frame.height,
            options.compatibility,
            &options.limits,
        )?;
        for (pixel, alpha) in rgba.chunks_exact_mut(4).zip(plane) {
            pixel[3] = alpha;
        }
    }
    Ok(rgba)
}

fn clone_canvas(rgba: &[u8]) -> Result<Vec<u8>, DecodeError> {
    let mut copy = Vec::new();
    copy.try_reserve_exact(rgba.len())
        .map_err(|_| allocation_failed("cannot allocate decoded animation frame"))?;
    copy.extend_from_slice(rgba);
    Ok(copy)
}

fn allocation_failed(context: &'static str) -> DecodeError {
    DecodeError::new(DecodeErrorKind::AllocationFailed, None, context)
}

#[cfg(test)]
#[path = "decoder_tests.rs"]
mod decoder_tests;
