//! Existing lossless animation encode orchestration.

use crate::AnimationEncodeFrame;
use crate::AnimationEncodeOptions;
use crate::EncodeError;
use crate::Metadata;
use crate::vp8l;

const MAX_ANIMATION_DIMENSION: u32 = 1 << 24;
const MAX_ANIMATION_DURATION_MS: u32 = (1 << 24) - 1;

struct EncodedAnimationFrame {
    anmf_payload: Vec<u8>,
}

/// Encodes VP8L frame rectangles as a lossless WebP animation.
pub fn encode_lossless_animation(
    canvas_width: u32,
    canvas_height: u32,
    frames: &[AnimationEncodeFrame<'_>],
    options: AnimationEncodeOptions,
) -> Result<Vec<u8>, EncodeError> {
    encode_lossless_animation_with_metadata(
        canvas_width,
        canvas_height,
        frames,
        options,
        &Metadata::default(),
    )
}

/// Encodes VP8L frame rectangles as a lossless WebP animation with raw metadata.
pub fn encode_lossless_animation_with_metadata(
    canvas_width: u32,
    canvas_height: u32,
    frames: &[AnimationEncodeFrame<'_>],
    options: AnimationEncodeOptions,
    metadata: &Metadata,
) -> Result<Vec<u8>, EncodeError> {
    if canvas_width == 0
        || canvas_height == 0
        || canvas_width > MAX_ANIMATION_DIMENSION
        || canvas_height > MAX_ANIMATION_DIMENSION
        || frames.is_empty()
    {
        return Err(EncodeError::invalid_animation());
    }

    let mut encoded_frames = Vec::new();
    encoded_frames
        .try_reserve_exact(frames.len())
        .map_err(|_| EncodeError::allocation_failed())?;
    let mut has_alpha = false;
    for frame in frames {
        validate_animation_frame(canvas_width, canvas_height, frame)?;
        let (payload, frame_has_alpha) =
            vp8l::encode_vp8l_payload(frame.width, frame.height, frame.rgba)?;
        has_alpha |= frame_has_alpha;
        encoded_frames.push(EncodedAnimationFrame {
            anmf_payload: make_anmf_payload(frame, &payload)?,
        });
    }
    wrap_lossless_animation(
        canvas_width,
        canvas_height,
        options,
        has_alpha,
        encoded_frames,
        metadata,
    )
}

fn validate_animation_frame(
    canvas_width: u32,
    canvas_height: u32,
    frame: &AnimationEncodeFrame<'_>,
) -> Result<(), EncodeError> {
    if frame.x & 1 != 0
        || frame.y & 1 != 0
        || frame.duration_ms > MAX_ANIMATION_DURATION_MS
        || frame.x > 0x01ff_fffe
        || frame.y > 0x01ff_fffe
    {
        return Err(EncodeError::invalid_animation());
    }
    vp8l::validate_input(frame.width, frame.height, frame.rgba)?;
    let right = frame
        .x
        .checked_add(frame.width)
        .ok_or_else(EncodeError::invalid_animation)?;
    let bottom = frame
        .y
        .checked_add(frame.height)
        .ok_or_else(EncodeError::invalid_animation)?;
    if right > canvas_width || bottom > canvas_height {
        return Err(EncodeError::invalid_animation());
    }
    Ok(())
}

fn make_anmf_payload(
    frame: &AnimationEncodeFrame<'_>,
    vp8l_payload: &[u8],
) -> Result<Vec<u8>, EncodeError> {
    webp_mux::serialize_animation_frame(webp_mux::AnimationFrameMux {
        x: frame.x,
        y: frame.y,
        width: frame.width,
        height: frame.height,
        duration_ms: frame.duration_ms,
        dispose_to_background: frame.dispose_to_background,
        blend: frame.blend,
        vp8l_payload,
    })
    .map_err(map_container_error)
}

fn wrap_lossless_animation(
    width: u32,
    height: u32,
    options: AnimationEncodeOptions,
    has_alpha: bool,
    frames: Vec<EncodedAnimationFrame>,
    metadata: &Metadata,
) -> Result<Vec<u8>, EncodeError> {
    let frames = frames
        .into_iter()
        .map(|frame| frame.anmf_payload)
        .collect::<Vec<_>>();
    webp_mux::serialize_animation(
        width,
        height,
        webp_mux::AnimationMuxOptions {
            background_rgba: options.background_rgba,
            loop_count: options.loop_count,
        },
        has_alpha,
        &frames,
        metadata.borrowed(),
    )
    .map_err(map_container_error)
}

fn map_container_error(error: webp_mux::ContainerError) -> EncodeError {
    match error.kind() {
        webp_mux::ContainerErrorKind::SizeOverflow => EncodeError::output_size_overflow(),
        webp_mux::ContainerErrorKind::AllocationFailed => EncodeError::allocation_failed(),
        webp_mux::ContainerErrorKind::InvalidDimensions => EncodeError::invalid_dimensions(),
        webp_mux::ContainerErrorKind::InvalidAnimation => EncodeError::invalid_animation(),
        webp_mux::ContainerErrorKind::UnexpectedEof
        | webp_mux::ContainerErrorKind::InvalidContainer
        | webp_mux::ContainerErrorKind::LimitExceeded => EncodeError::output_size_overflow(),
    }
}
