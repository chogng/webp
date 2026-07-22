//! Public animation encoder models.

/// Global settings for a lossless WebP animation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AnimationEncodeOptions {
    pub background_rgba: [u8; 4],
    /// Number of loops; zero means infinitely many.
    pub loop_count: u16,
}

/// One rectangular frame of a lossless WebP animation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnimationEncodeFrame<'a> {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub duration_ms: u32,
    pub rgba: &'a [u8],
    pub dispose_to_background: bool,
    pub blend: bool,
}
