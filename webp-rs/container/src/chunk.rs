//! Borrowed RIFF chunk framing.

use crate::FourCc;

/// One top-level RIFF chunk, including the original padding byte when present.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Chunk<'a> {
    pub fourcc: FourCc,
    pub payload: &'a [u8],
    pub padding: Option<u8>,
    /// Byte offset of the chunk `FourCC` from the beginning of the input.
    pub offset: usize,
}

impl Chunk<'_> {
    #[must_use]
    pub fn is_known(&self) -> bool {
        crate::fourcc::is_known(self.fourcc)
    }
}

/// An owned RIFF chunk for constructing or editing a WebP container.
///
/// `MuxChunk` owns only container bytes. Codec payloads are intentionally
/// opaque, so using it never decodes or re-encodes VP8 or VP8L data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MuxChunk {
    fourcc: FourCc,
    payload: Vec<u8>,
}

impl MuxChunk {
    /// Creates an owned chunk with an opaque payload.
    #[must_use]
    pub fn new(fourcc: FourCc, payload: Vec<u8>) -> Self {
        Self { fourcc, payload }
    }

    /// Returns the chunk identifier.
    #[must_use]
    pub fn fourcc(&self) -> FourCc {
        self.fourcc
    }

    /// Returns the opaque payload bytes.
    #[must_use]
    pub fn payload(&self) -> &[u8] {
        &self.payload
    }
}
