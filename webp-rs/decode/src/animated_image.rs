//! Animated WebP decode orchestration.

use crate::DecodeError;
use crate::DecodeErrorKind;
use crate::DecodeOptions;
use crate::animation::AnimationCanvas;
use crate::animation::DecodedFrame;
use crate::checked_image_bytes;
use webp_demux::FrameBitstream;

/// Pixel layout produced by [`AnimationDecoder`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum AnimationColorMode {
    /// Straight red, green, blue, alpha bytes.
    #[default]
    Rgba,
    /// Straight blue, green, red, alpha bytes.
    Bgra,
    /// Red, green, blue, alpha bytes with color premultiplied by alpha.
    RgbaPremultiplied,
    /// Blue, green, red, alpha bytes with color premultiplied by alpha.
    BgraPremultiplied,
}

/// Configuration for [`AnimationDecoder`].
///
/// The default retains the single-threaded behavior of [`decode_animation`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AnimationDecoderOptions {
    /// Container policy and resource limits used for every frame.
    pub decode: DecodeOptions,
    /// Layout of the pixels returned by [`AnimationDecoder::next_frame`].
    pub color_mode: AnimationColorMode,
    /// Decode independent VP8/VP8L color and `ALPH` payloads concurrently.
    ///
    /// Composition remains ordered and occurs on the caller's thread. This
    /// option is therefore most useful for lossy frames that carry `ALPH`.
    pub use_threads: bool,
}

/// Immutable animation properties available before decoding any frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnimationInfo {
    pub width: u32,
    pub height: u32,
    /// `0` represents infinitely many loops.
    pub loop_count: u16,
    /// The ANIM background color converted to straight RGBA8.
    pub background_rgba: [u8; 4],
    pub frame_count: usize,
}

/// One display-ready canvas returned by [`AnimationDecoder::next_frame`].
///
/// `pixels` is owned by the decoder and remains valid until the next mutable
/// operation on that decoder. Rust's borrowing rules prevent retaining it
/// while requesting another frame or resetting the decoder.
#[derive(Debug, PartialEq, Eq)]
pub struct AnimationDecoderFrame<'a> {
    /// End time of this frame in milliseconds from the start of the animation.
    pub timestamp_ms: u64,
    /// Duration declared by this ANMF frame.
    pub duration_ms: u32,
    /// Layout of [`Self::pixels`].
    pub color_mode: AnimationColorMode,
    /// Full display canvas in [`Self::color_mode`].
    pub pixels: &'a [u8],
}

/// Stateful, display-order WebP animation decoder.
///
/// Unlike [`decode_animation`], this decoder retains one canvas and fetches
/// frames on demand. It borrows the complete input for its lifetime.
#[derive(Debug)]
pub struct AnimationDecoder<'a> {
    container: webp_demux::Container<'a>,
    canvas: AnimationCanvas,
    output: Option<Vec<u8>>,
    options: AnimationDecoderOptions,
    info: AnimationInfo,
    next_frame: usize,
    timestamp_ms: u64,
}

impl<'a> AnimationDecoder<'a> {
    /// Parses an animated container and prepares its first display frame.
    pub fn new(data: &'a [u8], options: AnimationDecoderOptions) -> Result<Self, DecodeError> {
        let container = crate::container_adapter::parse(
            data,
            options.decode.compatibility,
            &options.decode.limits,
        )?;
        let animation = container.animation().ok_or_else(|| {
            DecodeError::at(
                DecodeErrorKind::UnsupportedFeature,
                0,
                "AnimationDecoder requires an animated WebP container",
            )
        })?;
        let vp8x = container.vp8x().expect("animation requires VP8X");
        let background_rgba = [
            animation.background_bgra[2],
            animation.background_bgra[1],
            animation.background_bgra[0],
            animation.background_bgra[3],
        ];
        let info = AnimationInfo {
            width: vp8x.canvas_width,
            height: vp8x.canvas_height,
            loop_count: animation.loop_count,
            background_rgba,
            frame_count: animation.frame_count(),
        };
        let canvas = AnimationCanvas::new(
            info.width,
            info.height,
            animation.background_bgra,
            &options.decode.limits,
        )?;
        let output = output_buffer(
            options.color_mode,
            canvas.rgba().len(),
            &options.decode.limits,
        )?;
        Ok(Self {
            container,
            canvas,
            output,
            options,
            info,
            next_frame: 0,
            timestamp_ms: 0,
        })
    }

    /// Returns global animation properties without advancing the decoder.
    #[must_use]
    pub const fn info(&self) -> &AnimationInfo {
        &self.info
    }

    /// Returns the validated zero-copy container view owned by this decoder.
    #[must_use]
    pub const fn demuxer(&self) -> &webp_demux::Container<'a> {
        &self.container
    }

    /// Returns whether another display frame can be fetched.
    #[must_use]
    pub fn has_more_frames(&self) -> bool {
        self.next_frame < self.info.frame_count
    }

    /// Decodes and returns the next full display canvas, if any.
    pub fn next_frame(&mut self) -> Result<Option<AnimationDecoderFrame<'_>>, DecodeError> {
        let Some(frame) = self
            .container
            .animation()
            .and_then(|animation| animation.frame(self.next_frame))
            .copied()
        else {
            return Ok(None);
        };
        let rgba = decode_animation_frame(&frame, &self.options.decode, self.options.use_threads)?;
        self.canvas.compose(
            DecodedFrame {
                x: frame.x,
                y: frame.y,
                width: frame.width,
                height: frame.height,
                rgba: &rgba,
                blend: frame.blend,
                dispose_to_background: frame.dispose_to_background,
            },
            &self.options.decode.limits,
        )?;
        self.timestamp_ms = self
            .timestamp_ms
            .checked_add(u64::from(frame.duration_ms))
            .ok_or_else(|| {
                DecodeError::new(
                    DecodeErrorKind::LimitExceeded,
                    None,
                    "animation timestamp overflow",
                )
            })?;
        self.next_frame += 1;
        Ok(Some(AnimationDecoderFrame {
            timestamp_ms: self.timestamp_ms,
            duration_ms: frame.duration_ms,
            color_mode: self.options.color_mode,
            pixels: self.output_pixels(),
        }))
    }

    /// Restarts frame fetching from the first ANMF frame.
    pub fn reset(&mut self) {
        self.canvas.reset();
        self.next_frame = 0;
        self.timestamp_ms = 0;
    }

    fn output_pixels(&mut self) -> &[u8] {
        let Some(output) = self.output.as_mut() else {
            return self.canvas.rgba();
        };
        convert_canvas(self.canvas.rgba(), output, self.options.color_mode);
        output
    }
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

/// Decodes an animated WebP into display-ready straight-RGBA8 canvas frames.
///
/// Each returned frame contains the full canvas after blending and disposal.
/// Static images are rejected.
pub fn decode_animation(data: &[u8], options: &DecodeOptions) -> Result<Animation, DecodeError> {
    let mut decoder = AnimationDecoder::new(
        data,
        AnimationDecoderOptions {
            decode: options.clone(),
            use_threads: false,
            ..AnimationDecoderOptions::default()
        },
    )?;
    let info = *decoder.info();
    let canvas_bytes = checked_image_bytes(info.width, info.height, 4)?;
    let total_output_bytes = canvas_bytes.checked_mul(info.frame_count).ok_or_else(|| {
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

    let mut frames = Vec::new();
    frames
        .try_reserve_exact(info.frame_count)
        .map_err(|_| allocation_failed("cannot reserve decoded animation frames"))?;
    while let Some(frame) = decoder.next_frame()? {
        frames.push(AnimationFrame {
            duration_ms: frame.duration_ms,
            rgba: clone_canvas(frame.pixels)?,
        });
    }
    Ok(Animation {
        width: info.width,
        height: info.height,
        loop_count: info.loop_count,
        frames,
    })
}

fn decode_animation_frame(
    frame: &webp_demux::AnimationFrame<'_>,
    options: &DecodeOptions,
    use_threads: bool,
) -> Result<Vec<u8>, DecodeError> {
    let (mut rgba, alpha) = if use_threads && frame.alpha.is_some() {
        let (color, alpha) = std::thread::scope(|scope| {
            let color = scope.spawn(|| decode_animation_color(frame, options));
            let alpha = scope.spawn(|| decode_animation_alpha(frame, options));
            (color.join(), alpha.join())
        });
        (join_frame_task(color)?, join_frame_task(alpha)?)
    } else {
        (
            decode_animation_color(frame, options)?,
            decode_animation_alpha(frame, options)?,
        )
    };
    if let Some(alpha) = alpha {
        for (pixel, alpha) in rgba.chunks_exact_mut(4).zip(alpha) {
            pixel[3] = alpha;
        }
    }
    Ok(rgba)
}

fn decode_animation_color(
    frame: &webp_demux::AnimationFrame<'_>,
    options: &DecodeOptions,
) -> Result<Vec<u8>, DecodeError> {
    match frame.bitstream {
        FrameBitstream::Vp8l(payload) => {
            let decoded = crate::vp8l::image_reader::decode_vp8l(payload, &options.limits)?;
            if (decoded.header.width, decoded.header.height) != (frame.width, frame.height) {
                return Err(DecodeError::new(
                    DecodeErrorKind::InvalidContainer,
                    None,
                    "ANMF VP8L dimensions do not match its frame rectangle",
                ));
            }
            Ok(decoded.rgba)
        }
        FrameBitstream::Vp8(payload) => {
            let header = crate::vp8::parse_riff_payload(
                payload,
                Some((frame.width, frame.height)),
                &options.limits,
            )?;
            Ok(
                crate::vp8::decode_intra_frame(payload, &header, &options.limits)?
                    .to_rgba(&options.limits)?,
            )
        }
    }
}

fn decode_animation_alpha(
    frame: &webp_demux::AnimationFrame<'_>,
    options: &DecodeOptions,
) -> Result<Option<Vec<u8>>, DecodeError> {
    frame.alpha.map_or(Ok(None), |alpha| {
        crate::alpha::decode::decode(
            alpha,
            frame.width,
            frame.height,
            options.compatibility,
            &options.limits,
        )
        .map(Some)
    })
}

fn join_frame_task<T>(
    result: std::thread::Result<Result<T, DecodeError>>,
) -> Result<T, DecodeError> {
    result.map_err(|_| {
        DecodeError::new(
            DecodeErrorKind::InvalidBitstream,
            None,
            "animation frame worker panicked",
        )
    })?
}

fn output_buffer(
    color_mode: AnimationColorMode,
    length: usize,
    limits: &crate::DecodeLimits,
) -> Result<Option<Vec<u8>>, DecodeError> {
    if color_mode == AnimationColorMode::Rgba {
        return Ok(None);
    }
    let retained = length.checked_mul(2).ok_or_else(|| {
        DecodeError::new(
            DecodeErrorKind::LimitExceeded,
            None,
            "animation output canvas size overflow",
        )
    })?;
    if retained > limits.max_alloc_bytes {
        return Err(DecodeError::new(
            DecodeErrorKind::LimitExceeded,
            None,
            "animation canvases exceed configured allocation limit",
        ));
    }
    let mut output = Vec::new();
    output.try_reserve_exact(length).map_err(|_| {
        DecodeError::new(
            DecodeErrorKind::AllocationFailed,
            None,
            "cannot reserve animation output canvas",
        )
    })?;
    output.resize(length, 0);
    Ok(Some(output))
}

fn convert_canvas(source: &[u8], output: &mut [u8], color_mode: AnimationColorMode) {
    debug_assert_eq!(source.len(), output.len());
    for (source, output) in source.chunks_exact(4).zip(output.chunks_exact_mut(4)) {
        let alpha = source[3];
        let (red, green, blue) = match color_mode {
            AnimationColorMode::Rgba | AnimationColorMode::Bgra => {
                (source[0], source[1], source[2])
            }
            AnimationColorMode::RgbaPremultiplied | AnimationColorMode::BgraPremultiplied => (
                premultiply(source[0], alpha),
                premultiply(source[1], alpha),
                premultiply(source[2], alpha),
            ),
        };
        match color_mode {
            AnimationColorMode::Rgba | AnimationColorMode::RgbaPremultiplied => {
                output.copy_from_slice(&[red, green, blue, alpha]);
            }
            AnimationColorMode::Bgra | AnimationColorMode::BgraPremultiplied => {
                output.copy_from_slice(&[blue, green, red, alpha]);
            }
        }
    }
}

fn premultiply(channel: u8, alpha: u8) -> u8 {
    const FRACTION_BITS: u32 = 24;
    let scale = u32::from(alpha) * ((1 << FRACTION_BITS) / 255);
    ((u32::from(channel) * scale + (1 << (FRACTION_BITS - 1))) >> FRACTION_BITS) as u8
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
