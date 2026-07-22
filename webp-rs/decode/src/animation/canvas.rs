use crate::DecodeError;
use crate::DecodeErrorKind;
use crate::DecodeLimits;
use crate::checked_image_bytes;
use crate::checked_rect_end;

/// One decoded straight-RGBA8 frame ready for canvas composition.
#[derive(Debug, Clone, Copy)]
pub struct DecodedFrame<'a> {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub rgba: &'a [u8],
    /// `true` alpha-blends the frame; `false` replaces its rectangle.
    pub blend: bool,
    /// Clear this frame's rectangle to the animation background before the
    /// following frame is composed.
    pub dispose_to_background: bool,
}

/// A WebP animation canvas that applies disposal before each following frame.
#[derive(Debug, Clone)]
pub struct AnimationCanvas {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
    background: [u8; 4],
    pending_disposal: Option<(u32, u32, u32, u32)>,
}

impl AnimationCanvas {
    /// Creates a canvas filled with WebP's BGRA background color.
    pub fn new(
        width: u32,
        height: u32,
        background_bgra: [u8; 4],
        limits: &DecodeLimits,
    ) -> Result<Self, DecodeError> {
        limits.check_image(width, height)?;
        let bytes = checked_image_bytes(width, height, 4)?;
        if bytes > limits.max_alloc_bytes {
            return Err(DecodeError::new(
                DecodeErrorKind::LimitExceeded,
                None,
                "animation canvas exceeds configured allocation limit",
            ));
        }
        let background = [
            background_bgra[2],
            background_bgra[1],
            background_bgra[0],
            background_bgra[3],
        ];
        let mut rgba = Vec::new();
        rgba.try_reserve_exact(bytes).map_err(|_| {
            DecodeError::new(
                DecodeErrorKind::AllocationFailed,
                None,
                "cannot reserve animation canvas",
            )
        })?;
        let pixels = usize::try_from(u64::from(width) * u64::from(height))
            .map_err(|_| invalid("animation canvas exceeds usize"))?;
        for _ in 0..pixels {
            rgba.extend_from_slice(&background);
        }
        Ok(Self {
            width,
            height,
            rgba,
            background,
            pending_disposal: None,
        })
    }

    #[must_use]
    pub const fn width(&self) -> u32 {
        self.width
    }

    #[must_use]
    pub const fn height(&self) -> u32 {
        self.height
    }

    #[must_use]
    pub fn rgba(&self) -> &[u8] {
        &self.rgba
    }

    /// Applies the prior frame's requested disposal, then renders `frame`.
    /// The canvas after this call is the image to display for `frame`.
    pub fn compose(
        &mut self,
        frame: DecodedFrame<'_>,
        limits: &DecodeLimits,
    ) -> Result<(), DecodeError> {
        checked_rect_end(frame.x, frame.width, self.width)?;
        checked_rect_end(frame.y, frame.height, self.height)?;
        let frame_bytes = checked_image_bytes(frame.width, frame.height, 4)?;
        if frame.rgba.len() != frame_bytes {
            return Err(DecodeError::new(
                DecodeErrorKind::InvalidParameter,
                None,
                "animation frame RGBA length does not match its rectangle",
            ));
        }
        let mut work = limits.work_budget();
        let frame_pixels = u64::from(frame.width) * u64::from(frame.height);
        let disposal_pixels = self
            .pending_disposal
            .map(|(_, _, width, height)| u64::from(width) * u64::from(height))
            .unwrap_or(0);
        work.consume(
            frame_pixels
                .checked_add(disposal_pixels)
                .ok_or_else(|| invalid("animation work exceeds u64"))?,
        )?;
        if let Some(rect) = self.pending_disposal.take() {
            self.fill_rect(rect);
        }
        let canvas_width =
            usize::try_from(self.width).map_err(|_| invalid("animation width exceeds usize"))?;
        let frame_width = usize::try_from(frame.width)
            .map_err(|_| invalid("animation frame width exceeds usize"))?;
        let x = usize::try_from(frame.x).map_err(|_| invalid("animation x exceeds usize"))?;
        let y = usize::try_from(frame.y).map_err(|_| invalid("animation y exceeds usize"))?;
        for row in 0..usize::try_from(frame.height)
            .map_err(|_| invalid("animation frame height exceeds usize"))?
        {
            let dst_start = ((y + row) * canvas_width + x) * 4;
            let src_start = row * frame_width * 4;
            for (dst, src) in self.rgba[dst_start..dst_start + frame_width * 4]
                .chunks_exact_mut(4)
                .zip(frame.rgba[src_start..src_start + frame_width * 4].chunks_exact(4))
            {
                if frame.blend {
                    blend(src, dst);
                } else {
                    dst.copy_from_slice(src);
                }
            }
        }
        self.pending_disposal =
            frame
                .dispose_to_background
                .then_some((frame.x, frame.y, frame.width, frame.height));
        Ok(())
    }

    fn fill_rect(&mut self, (x, y, width, height): (u32, u32, u32, u32)) {
        let canvas_width = self.width as usize;
        let x = x as usize;
        let width = width as usize;
        for row in y as usize..(y + height) as usize {
            let start = (row * canvas_width + x) * 4;
            self.rgba[start..start + width * 4]
                .chunks_exact_mut(4)
                .for_each(|pixel| pixel.copy_from_slice(&self.background));
        }
    }
}

fn invalid(context: &'static str) -> DecodeError {
    DecodeError::new(DecodeErrorKind::InvalidParameter, None, context)
}

fn blend(source: &[u8], destination: &mut [u8]) {
    let source_alpha = u32::from(source[3]);
    let destination_alpha = u32::from(destination[3]);
    let inverse_source_alpha = 255 - source_alpha;
    let output_alpha = source_alpha + div_255(destination_alpha * inverse_source_alpha);
    if output_alpha == 0 {
        destination.copy_from_slice(&[0; 4]);
        return;
    }
    for channel in 0..3 {
        let numerator = u32::from(source[channel]) * source_alpha * 255
            + u32::from(destination[channel]) * destination_alpha * inverse_source_alpha;
        destination[channel] = ((numerator + output_alpha * 127) / (output_alpha * 255)) as u8;
    }
    destination[3] = output_alpha as u8;
}

fn div_255(value: u32) -> u32 {
    (value + 127) / 255
}

#[cfg(test)]
#[path = "canvas_tests.rs"]
mod tests;
