//! Borrowed ANIM/ANMF wire models.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Animation<'a> {
    pub background_bgra: [u8; 4],
    pub loop_count: u16,
    pub(crate) frames: Vec<AnimationFrame<'a>>,
}

impl<'a> Animation<'a> {
    #[must_use]
    pub fn frames(&self) -> &[AnimationFrame<'a>] {
        &self.frames
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnimationFrame<'a> {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub duration_ms: u32,
    pub dispose_to_background: bool,
    pub blend: bool,
    pub alpha: Option<&'a [u8]>,
    pub bitstream: FrameBitstream<'a>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameBitstream<'a> {
    Vp8(&'a [u8]),
    Vp8l(&'a [u8]),
}
