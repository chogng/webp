//! Public decode policy composition.

use crate::CompatibilityProfile;
use crate::DecodeLimits;

/// Configuration for WebP decoding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodeOptions {
    pub limits: DecodeLimits,
    pub compatibility: CompatibilityProfile,
}

impl Default for DecodeOptions {
    fn default() -> Self {
        Self {
            limits: DecodeLimits::default(),
            compatibility: CompatibilityProfile::SpecStrict,
        }
    }
}
