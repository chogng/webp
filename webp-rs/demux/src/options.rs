//! Demux policy and resource limits.

/// Parsing policy for inputs where the WebP specification and legacy readers
/// intentionally differ.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum CompatibilityProfile {
    /// Reject malformed containers according to the format specification.
    #[default]
    SpecStrict,
    /// Accept selected legacy forms supported by libwebp.
    LibwebpCompatible,
}

/// Options controlling one zero-copy demux operation.
///
/// The profile determines which legacy layout quirks are accepted. Limits are
/// owned by the caller so the same policy can be reused for multiple inputs.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DemuxOptions {
    pub profile: CompatibilityProfile,
    pub limits: ContainerLimits,
}

/// Resource bounds applied while parsing RIFF layout and animation metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContainerLimits {
    pub max_input_bytes: usize,
    pub max_width: u32,
    pub max_height: u32,
    pub max_pixels: u64,
    pub max_frames: u32,
    pub max_total_frame_pixels: u64,
    pub max_metadata_bytes: usize,
    /// Maximum number of top-level RIFF chunks retained by a demux result.
    pub max_chunks: u32,
}

impl Default for ContainerLimits {
    fn default() -> Self {
        Self {
            max_input_bytes: 512 * 1024 * 1024,
            max_width: 16_777_216,
            max_height: 16_777_216,
            max_pixels: 268_435_456,
            max_frames: 16_384,
            max_total_frame_pixels: 1_073_741_824,
            max_metadata_bytes: 64 * 1024 * 1024,
            max_chunks: 65_536,
        }
    }
}
