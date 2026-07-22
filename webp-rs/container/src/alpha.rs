//! Validated `ALPH` header fields shared by payload reading and writing.

/// The compression method encoded in an `ALPH` header.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AlphaCompression {
    /// The payload contains filtered alpha bytes directly.
    Raw,
    /// The payload is a headerless VP8L stream whose green channel is alpha.
    Lossless,
}

/// Spatial filter applied to an alpha plane before it is written to the file.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AlphaFilter {
    None,
    Horizontal,
    Vertical,
    Gradient,
}

/// Informative preprocessing declared by an `ALPH` header.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum AlphaPreprocessing {
    #[default]
    None,
    LevelReduction,
}

/// Validated fields from the one-byte `ALPH` header.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AlphaHeader {
    pub compression: AlphaCompression,
    pub filter: AlphaFilter,
    pub preprocessing: AlphaPreprocessing,
}

impl AlphaHeader {
    /// Serializes the validated header fields with reserved bits cleared.
    #[must_use]
    pub const fn to_byte(self) -> u8 {
        let compression = match self.compression {
            AlphaCompression::Raw => 0,
            AlphaCompression::Lossless => 1,
        };
        let filter = match self.filter {
            AlphaFilter::None => 0,
            AlphaFilter::Horizontal => 1,
            AlphaFilter::Vertical => 2,
            AlphaFilter::Gradient => 3,
        };
        let preprocessing = match self.preprocessing {
            AlphaPreprocessing::None => 0,
            AlphaPreprocessing::LevelReduction => 1,
        };
        compression | (filter << 2) | (preprocessing << 4)
    }
}
