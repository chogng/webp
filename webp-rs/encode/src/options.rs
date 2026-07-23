//! Stable encoder options.

/// Explicit configuration for the bounded static lossy VP8 encoder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LossyEncodeOptions {
    /// VP8 quantization quality on a 0 (smallest output) through 100 scale.
    pub quality: u8,
}

impl Default for LossyEncodeOptions {
    fn default() -> Self {
        Self { quality: 75 }
    }
}

/// Stable size/decoding-latency tradeoffs for static lossless encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum LosslessEncodeProfile {
    #[default]
    Default,
    /// Spend bounded extra encode work to minimize the lossless file size.
    HighCompression,
    /// Prefer compact spatial Huffman groups.
    FastDecodeCompact,
    /// Prefer fewer, larger spatial Huffman groups.
    FastDecodeLowLatency,
}

/// Options for static lossless WebP encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct LosslessEncodeOptions {
    /// Selects the bounded encoding portfolio.
    pub profile: LosslessEncodeProfile,
}
