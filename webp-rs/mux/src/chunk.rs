//! Owned RIFF chunks used for construction and editing.

use crate::FourCc;

/// An owned RIFF chunk for constructing or editing a WebP container.
///
/// Codec payloads remain opaque, so using this type never decodes or
/// re-encodes VP8 or VP8L data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MuxChunk {
    fourcc: FourCc,
    payload: Vec<u8>,
}

impl MuxChunk {
    #[must_use]
    pub fn new(fourcc: FourCc, payload: Vec<u8>) -> Self {
        Self { fourcc, payload }
    }

    #[must_use]
    pub fn fourcc(&self) -> FourCc {
        self.fourcc
    }

    #[must_use]
    pub fn payload(&self) -> &[u8] {
        &self.payload
    }
}
