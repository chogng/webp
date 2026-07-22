//! Lossless container edits over owned opaque chunks.

use crate::ANMF;
use crate::AnimationFrameInput;
use crate::ContainerError;
use crate::DemuxOptions;
use crate::EXIF;
use crate::ICCP;
use crate::Metadata;
use crate::MuxChunk;
use crate::Muxer;
use crate::XMP;
use crate::mux::finish_chunks;
use crate::mux::remove_metadata_chunk;
use crate::mux::serialize_animation_frame_input;
use crate::mux::set_metadata_chunk;
use crate::mux::set_vp8x_flag;

/// An editable WebP container that preserves opaque codec and unknown chunks.
///
/// Construction parses according to the supplied [`DemuxOptions`]. Editing
/// copies only the container chunks; it never decodes or re-encodes image
/// payloads. Strict input round-trips byte-for-byte when unchanged; compatible
/// trailing data and non-zero padding are normalized. [`Editor::finish`]
/// validates the resulting strict container layout.
#[derive(Debug)]
pub struct Editor {
    chunks: Vec<MuxChunk>,
}

impl Editor {
    /// Parses a container into an owned, losslessly editable chunk sequence.
    pub fn parse(data: &[u8], options: &DemuxOptions) -> Result<Self, ContainerError> {
        let demuxer = crate::Demuxer::parse(data, options)?;
        let mut chunks = Vec::new();
        chunks
            .try_reserve_exact(demuxer.chunk_count())
            .map_err(|_| crate::mux::allocation_failed())?;
        for chunk in demuxer.chunks() {
            chunks.push(MuxChunk::new(chunk.fourcc, chunk.payload.to_vec()));
        }
        Ok(Self { chunks })
    }

    /// Returns the owned chunks in their preserved wire order.
    #[must_use]
    pub fn chunks(&self) -> &[MuxChunk] {
        &self.chunks
    }

    /// Returns the current raw metadata payloads.
    #[must_use]
    pub fn metadata(&self) -> Metadata<'_> {
        Metadata {
            iccp: payload(&self.chunks, ICCP),
            exif: payload(&self.chunks, EXIF),
            xmp: payload(&self.chunks, XMP),
        }
    }

    /// Replaces or adds ICC profile metadata and synchronizes `VP8X` flags.
    pub fn set_iccp(&mut self, payload: Vec<u8>) -> Result<&mut Self, ContainerError> {
        set_metadata_chunk(&mut self.chunks, ICCP, payload)?;
        Ok(self)
    }

    /// Replaces or adds EXIF metadata and synchronizes `VP8X` flags.
    pub fn set_exif(&mut self, payload: Vec<u8>) -> Result<&mut Self, ContainerError> {
        set_metadata_chunk(&mut self.chunks, EXIF, payload)?;
        Ok(self)
    }

    /// Replaces or adds XMP metadata and synchronizes `VP8X` flags.
    pub fn set_xmp(&mut self, payload: Vec<u8>) -> Result<&mut Self, ContainerError> {
        set_metadata_chunk(&mut self.chunks, XMP, payload)?;
        Ok(self)
    }

    /// Removes ICC profile metadata, returning whether a chunk was removed.
    pub fn remove_iccp(&mut self) -> Result<bool, ContainerError> {
        remove_metadata_chunk(&mut self.chunks, ICCP)
    }

    /// Removes EXIF metadata, returning whether a chunk was removed.
    pub fn remove_exif(&mut self) -> Result<bool, ContainerError> {
        remove_metadata_chunk(&mut self.chunks, EXIF)
    }

    /// Removes XMP metadata, returning whether a chunk was removed.
    pub fn remove_xmp(&mut self) -> Result<bool, ContainerError> {
        remove_metadata_chunk(&mut self.chunks, XMP)
    }

    /// Replaces one top-level chunk while preserving its position.
    pub fn replace_chunk(&mut self, index: usize, chunk: MuxChunk) -> Option<MuxChunk> {
        self.chunks
            .get_mut(index)
            .map(|current| core::mem::replace(current, chunk))
    }

    /// Replaces one ANMF frame without decoding its image payload.
    pub fn replace_animation_frame(
        &mut self,
        frame_index: usize,
        frame: AnimationFrameInput<'_>,
    ) -> Result<bool, ContainerError> {
        let Some(index) = self
            .chunks
            .iter()
            .enumerate()
            .filter(|(_, chunk)| chunk.fourcc() == ANMF)
            .nth(frame_index)
            .map(|(index, _)| index)
        else {
            return Ok(false);
        };
        let payload = serialize_animation_frame_input(frame)?;
        if frame.alpha.is_some() {
            set_vp8x_flag(&mut self.chunks, 1 << 4, true)?;
        }
        self.chunks[index] = MuxChunk::new(ANMF, payload);
        Ok(true)
    }

    /// Serializes the edited, strict container without re-encoding codec data.
    pub fn finish(self) -> Result<Vec<u8>, ContainerError> {
        finish_chunks(&self.chunks)
    }

    /// Converts this editor into the same generic mux builder.
    #[must_use]
    pub fn into_muxer(self) -> Muxer {
        Muxer::from_chunks(self.chunks)
    }
}

fn payload(chunks: &[MuxChunk], fourcc: [u8; 4]) -> Option<&[u8]> {
    chunks
        .iter()
        .find(|chunk| chunk.fourcc() == fourcc)
        .map(MuxChunk::payload)
}

#[cfg(test)]
#[path = "editor_tests.rs"]
mod tests;
