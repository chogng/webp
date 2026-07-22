//! Stable codec options.

#[cfg(feature = "decode")]
use crate::CompatibilityProfile;
#[cfg(feature = "decode")]
use crate::DecodeLimits;

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
