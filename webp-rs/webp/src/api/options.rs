//! Stable codec options.

#[cfg(feature = "decode")]
use crate::CompatibilityProfile;
#[cfg(feature = "decode")]
use crate::DecodeLimits;

#[cfg(feature = "encode")]
/// Explicit configuration for the bounded static lossy VP8 encoder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LossyEncodeOptions {
    /// VP8 quantization quality on a 0 (smallest output) through 100 scale.
    pub quality: u8,
}

#[cfg(feature = "encode")]
impl Default for LossyEncodeOptions {
    fn default() -> Self {
        Self { quality: 75 }
    }
}

#[cfg(feature = "encode")]
/// Stable size/decoding-latency tradeoffs for static lossless encoding.
///
/// Every profile emits an ordinary VP8L bitstream. The fast-decode profiles
/// use coarse spatial Huffman groups and are never selected implicitly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum LosslessEncodeProfile {
    /// Preserve the encoder's established output and behavior.
    #[default]
    Default,
    /// Use 128-pixel spatial blocks and at most 64 entropy groups, falling
    /// back byte-for-byte unless the complete coarse file is smaller.
    FastDecodeCompact,
    /// Use 256-pixel spatial blocks and at most 16 entropy groups, falling
    /// back byte-for-byte unless the complete coarse file is smaller.
    FastDecodeLowLatency,
}

#[cfg(feature = "encode")]
/// Options for static lossless WebP encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct LosslessEncodeOptions {
    /// Selects the encoder's lossless size/decoding-latency tradeoff.
    pub profile: LosslessEncodeProfile,
}

#[cfg(feature = "decode")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodeOptions {
    pub limits: DecodeLimits,
    pub compatibility: CompatibilityProfile,
}

#[cfg(feature = "decode")]
impl Default for DecodeOptions {
    fn default() -> Self {
        Self {
            limits: DecodeLimits::default(),
            compatibility: CompatibilityProfile::SpecStrict,
        }
    }
}
