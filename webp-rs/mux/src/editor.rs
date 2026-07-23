//! Lossless container edits composed from demuxing and muxing.

use crate::ANMF;
use crate::ContainerError;
use crate::DemuxOptions;
use crate::EXIF;
use crate::ICCP;
use crate::Metadata;
use crate::MuxChunk;
use crate::Muxer;
use crate::VP8;
use crate::VP8L;
use crate::XMP;
use crate::frame::AnimationFrameInput;
use crate::frame::serialize_animation_frame_input;
use crate::state::frame_uses_alpha;
use crate::state::insert_animation_frame;
use crate::state::insert_chunk;
use crate::state::remove_animation_frame;
use crate::state::remove_chunk;
use crate::state::remove_metadata_chunk;
use crate::state::replace_chunk;
use crate::state::set_animation;
use crate::state::set_animation_params;
use crate::state::set_canvas_size;
use crate::state::set_metadata_chunk;
use crate::state::set_static_image;
use crate::state::set_vp8x_flag;
use crate::wire::copy_bytes;
use crate::wire::finish_chunks;

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
            .map_err(|_| crate::wire::allocation_failed())?;
        for chunk in demuxer.chunks() {
            chunks.push(MuxChunk::new(chunk.fourcc, copy_bytes(chunk.payload)?));
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
        replace_chunk(&mut self.chunks, index, chunk)
    }

    /// Inserts one top-level chunk at an exact wire-order position.
    pub fn insert_chunk(
        &mut self,
        index: usize,
        chunk: MuxChunk,
    ) -> Result<&mut Self, ContainerError> {
        insert_chunk(&mut self.chunks, index, chunk)?;
        Ok(self)
    }

    /// Removes and returns one top-level chunk.
    pub fn remove_chunk(&mut self, index: usize) -> Option<MuxChunk> {
        remove_chunk(&mut self.chunks, index)
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
        if frame_uses_alpha(frame) {
            set_vp8x_flag(&mut self.chunks, 1 << 4, true)?;
        }
        self.chunks[index] = MuxChunk::new(ANMF, payload);
        Ok(true)
    }

    /// Appends a validated ANMF frame without decoding its image payload.
    pub fn add_animation_frame(
        &mut self,
        frame: AnimationFrameInput<'_>,
    ) -> Result<&mut Self, ContainerError> {
        let frame_index = self
            .chunks
            .iter()
            .filter(|chunk| chunk.fourcc() == ANMF)
            .count();
        insert_animation_frame(&mut self.chunks, frame_index, frame)?;
        Ok(self)
    }

    /// Inserts a validated ANMF frame at a display-order position.
    pub fn insert_animation_frame(
        &mut self,
        frame_index: usize,
        frame: AnimationFrameInput<'_>,
    ) -> Result<&mut Self, ContainerError> {
        insert_animation_frame(&mut self.chunks, frame_index, frame)?;
        Ok(self)
    }

    /// Removes one ANMF frame by display-order position.
    pub fn remove_animation_frame(&mut self, frame_index: usize) -> Option<MuxChunk> {
        remove_animation_frame(&mut self.chunks, frame_index)
    }

    /// Replaces or adds animation background and loop controls.
    pub fn set_animation_params(
        &mut self,
        background_rgba: [u8; 4],
        loop_count: u16,
    ) -> Result<&mut Self, ContainerError> {
        set_animation_params(&mut self.chunks, background_rgba, loop_count)?;
        Ok(self)
    }

    /// Updates the `VP8X` canvas dimensions.
    pub fn set_canvas_size(
        &mut self,
        width: u32,
        height: u32,
    ) -> Result<&mut Self, ContainerError> {
        set_canvas_size(&mut self.chunks, width, height)?;
        Ok(self)
    }

    /// Replaces all image/animation chunks with one VP8L still image.
    pub fn set_static_vp8l(
        &mut self,
        width: u32,
        height: u32,
        payload: Vec<u8>,
        has_alpha: bool,
    ) -> Result<&mut Self, ContainerError> {
        set_static_image(
            &mut self.chunks,
            width,
            height,
            VP8L,
            payload,
            None,
            has_alpha,
        )?;
        Ok(self)
    }

    /// Replaces all image/animation chunks with one VP8 still image.
    pub fn set_static_vp8(
        &mut self,
        width: u32,
        height: u32,
        payload: Vec<u8>,
        alpha: Option<Vec<u8>>,
    ) -> Result<&mut Self, ContainerError> {
        let has_alpha = alpha.is_some();
        set_static_image(
            &mut self.chunks,
            width,
            height,
            VP8,
            payload,
            alpha,
            has_alpha,
        )?;
        Ok(self)
    }

    /// Replaces all image/animation chunks with empty animation controls.
    /// Existing metadata and unknown chunks are preserved.
    pub fn set_animation(
        &mut self,
        width: u32,
        height: u32,
        background_rgba: [u8; 4],
        loop_count: u16,
    ) -> Result<&mut Self, ContainerError> {
        set_animation(&mut self.chunks, width, height, background_rgba, loop_count)?;
        Ok(self)
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
