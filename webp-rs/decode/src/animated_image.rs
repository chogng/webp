//! Animated WebP decode orchestration.

use crate::Animation;
use crate::AnimationFrame;
use crate::DecodeError;
use crate::DecodeErrorKind;
use crate::DecodeOptions;
use crate::animation::AnimationCanvas;
use crate::animation::DecodedFrame;
use crate::checked_image_bytes;
use webp_demux::FrameBitstream;

pub(crate) fn decode_animation(
    data: &[u8],
    options: &DecodeOptions,
) -> Result<Animation, DecodeError> {
    let container = crate::container_adapter::parse(data, options.compatibility, &options.limits)?;
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
    frame: &webp_demux::AnimationFrame<'_>,
    options: &DecodeOptions,
) -> Result<Vec<u8>, DecodeError> {
    let mut rgba = match frame.bitstream {
        FrameBitstream::Vp8l(payload) => {
            let decoded = crate::vp8l::image_reader::decode_vp8l(payload, &options.limits)?;
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
            let header = crate::vp8::parse_riff_payload(
                payload,
                Some((frame.width, frame.height)),
                &options.limits,
            )?;
            crate::vp8::decode_intra_frame(payload, &header, &options.limits)?
                .to_rgba(&options.limits)?
        }
    };
    if let Some(alpha) = frame.alpha {
        let plane = crate::alpha::decode::decode(
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
#[path = "animated_image_tests.rs"]
mod decoder_tests;
