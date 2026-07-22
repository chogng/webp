//! Extended-canvas wire declarations shared by muxing and demuxing.

/// Parsed contents of the fixed-size `VP8X` chunk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Vp8x {
    pub flags: Vp8xFlags,
    pub canvas_width: u32,
    pub canvas_height: u32,
}

/// Feature flags declared by `VP8X`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Vp8xFlags(u8);

impl Vp8xFlags {
    const ICCP: u8 = 1 << 5;
    const ALPHA: u8 = 1 << 4;
    const EXIF: u8 = 1 << 3;
    const XMP: u8 = 1 << 2;
    const ANIMATION: u8 = 1 << 1;
    const RESERVED: u8 = (1 << 7) | (1 << 6) | 1;

    /// Constructs feature flags from their validated wire byte.
    #[doc(hidden)]
    #[must_use]
    pub const fn from_bits(bits: u8) -> Self {
        Self(bits)
    }

    #[must_use]
    pub fn iccp(self) -> bool {
        self.0 & Self::ICCP != 0
    }
    #[must_use]
    pub fn alpha(self) -> bool {
        self.0 & Self::ALPHA != 0
    }
    #[must_use]
    pub fn exif(self) -> bool {
        self.0 & Self::EXIF != 0
    }
    #[must_use]
    pub fn xmp(self) -> bool {
        self.0 & Self::XMP != 0
    }
    #[must_use]
    pub fn animation(self) -> bool {
        self.0 & Self::ANIMATION != 0
    }
    #[must_use]
    pub fn reserved_bits(self) -> u8 {
        self.0 & Self::RESERVED
    }
    #[must_use]
    pub fn bits(self) -> u8 {
        self.0
    }
}
