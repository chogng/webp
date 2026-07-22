//! Public animation models.

#[cfg(feature = "animation")]
/// A fully composed frame in display order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnimationFrame {
    /// The frame's declared display time in milliseconds.
    pub duration_ms: u32,
    /// Complete canvas contents after blending and disposal, in straight RGBA8.
    pub rgba: Vec<u8>,
}

#[cfg(feature = "animation")]
/// A decoded WebP animation with display-ready canvas frames.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Animation {
    pub width: u32,
    pub height: u32,
    /// `0` represents infinitely many loops.
    pub loop_count: u16,
    pub frames: Vec<AnimationFrame>,
}

#[cfg(all(feature = "animation", feature = "encode"))]
/// Global settings for a lossless WebP animation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AnimationEncodeOptions {
    /// Canvas fill color used by dispose-to-background frames, in straight RGBA8.
    pub background_rgba: [u8; 4],
    /// Number of animation loops; `0` represents infinitely many loops.
    pub loop_count: u16,
}

#[cfg(all(feature = "animation", feature = "encode"))]
/// One rectangle of a lossless WebP animation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnimationEncodeFrame<'a> {
    /// Even horizontal canvas offset in pixels.
    pub x: u32,
    /// Even vertical canvas offset in pixels.
    pub y: u32,
    pub width: u32,
    pub height: u32,
    /// Display duration in milliseconds, representable as an unsigned 24-bit value.
    pub duration_ms: u32,
    /// Straight/unpremultiplied RGBA8 pixels in row-major frame-rectangle order.
    pub rgba: &'a [u8],
    /// Restore this rectangle to the configured background after display.
    pub dispose_to_background: bool,
    /// Blend this frame over the current canvas; `false` overwrites the rectangle.
    pub blend: bool,
}
