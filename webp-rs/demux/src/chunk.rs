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
        webp_container::is_known(self.fourcc)
    }
}
