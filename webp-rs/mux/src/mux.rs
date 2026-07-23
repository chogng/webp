//! Owned RIFF serialization for public muxing and encoder adapters.

use crate::ALPH;
use crate::ANIM;
use crate::ANMF;
use crate::ContainerError;
use crate::ContainerErrorKind;
use crate::EXIF;
use crate::ICCP;
use crate::MuxChunk;
use crate::VP8;
use crate::VP8L;
use crate::VP8X;
use crate::XMP;
use crate::frame::AnimationFrameInput;
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
use crate::wire::allocation_failed;
use crate::wire::copy_bytes;
use crate::wire::dimensions_fit_u24_minus_one;
use crate::wire::error;
use crate::wire::finish_chunks;

/// Builds a strict WebP RIFF container from owned, opaque chunks.
///
/// Common static and animation constructors establish `VP8X` geometry and
/// flags for callers. [`Muxer::add_chunk`] is also available for extensions
/// and unknown chunks; [`Muxer::finish`] validates the resulting layout.
#[derive(Debug, Default)]
pub struct Muxer {
    chunks: Vec<MuxChunk>,
}

impl Muxer {
    /// Creates an empty generic container builder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the current owned chunks in their output order.
    #[must_use]
    pub fn chunks(&self) -> &[MuxChunk] {
        &self.chunks
    }

    /// Creates an extended static VP8L container with known canvas geometry.
    pub fn static_vp8l(
        width: u32,
        height: u32,
        payload: Vec<u8>,
        has_alpha: bool,
    ) -> Result<Self, ContainerError> {
        let mut muxer = Self::with_canvas(width, height, u8::from(has_alpha) << 4)?;
        muxer.add_chunk(MuxChunk::new(VP8L, payload))?;
        Ok(muxer)
    }

    /// Creates an extended static VP8 container with an optional ALPH payload.
    pub fn static_vp8(
        width: u32,
        height: u32,
        payload: Vec<u8>,
        alpha: Option<Vec<u8>>,
    ) -> Result<Self, ContainerError> {
        let mut muxer = Self::with_canvas(width, height, u8::from(alpha.is_some()) << 4)?;
        if let Some(alpha) = alpha {
            muxer.add_chunk(MuxChunk::new(ALPH, alpha))?;
        }
        muxer.add_chunk(MuxChunk::new(VP8, payload))?;
        Ok(muxer)
    }

    /// Creates an animated container and writes its `VP8X` and `ANIM` chunks.
    pub fn animation(
        width: u32,
        height: u32,
        background_rgba: [u8; 4],
        loop_count: u16,
    ) -> Result<Self, ContainerError> {
        let mut muxer = Self::with_canvas(width, height, 1 << 1)?;
        let control = [
            background_rgba[2],
            background_rgba[1],
            background_rgba[0],
            background_rgba[3],
            loop_count.to_le_bytes()[0],
            loop_count.to_le_bytes()[1],
        ];
        muxer.add_chunk(MuxChunk::new(ANIM, copy_bytes(&control)?))?;
        Ok(muxer)
    }

    /// Adds a top-level opaque chunk. Unknown chunks are serialized unchanged.
    pub fn add_chunk(&mut self, chunk: MuxChunk) -> Result<&mut Self, ContainerError> {
        self.chunks
            .try_reserve(1)
            .map_err(|_| allocation_failed())?;
        self.chunks.push(chunk);
        Ok(self)
    }

    /// Inserts an opaque top-level chunk at an exact output position.
    pub fn insert_chunk(
        &mut self,
        index: usize,
        chunk: MuxChunk,
    ) -> Result<&mut Self, ContainerError> {
        insert_chunk(&mut self.chunks, index, chunk)?;
        Ok(self)
    }

    /// Replaces one top-level chunk while preserving its position.
    pub fn replace_chunk(&mut self, index: usize, chunk: MuxChunk) -> Option<MuxChunk> {
        replace_chunk(&mut self.chunks, index, chunk)
    }

    /// Removes and returns one top-level chunk.
    pub fn remove_chunk(&mut self, index: usize) -> Option<MuxChunk> {
        remove_chunk(&mut self.chunks, index)
    }

    /// Adds an opaque chunk while retaining builder chaining.
    pub fn with_chunk(mut self, chunk: MuxChunk) -> Result<Self, ContainerError> {
        self.add_chunk(chunk)?;
        Ok(self)
    }

    /// Adds a validated ANMF frame to an animated container.
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

    /// Sets the animation background and loop count, adding `ANIM` when needed.
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

    /// Replaces or adds ICC profile metadata.
    pub fn set_iccp(&mut self, payload: Vec<u8>) -> Result<&mut Self, ContainerError> {
        set_metadata_chunk(&mut self.chunks, ICCP, payload)?;
        Ok(self)
    }

    /// Replaces or adds EXIF metadata.
    pub fn set_exif(&mut self, payload: Vec<u8>) -> Result<&mut Self, ContainerError> {
        set_metadata_chunk(&mut self.chunks, EXIF, payload)?;
        Ok(self)
    }

    /// Replaces or adds XMP metadata.
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

    /// Serializes and strictly validates the finished RIFF container.
    pub fn finish(self) -> Result<Vec<u8>, ContainerError> {
        finish_chunks(&self.chunks)
    }

    fn with_canvas(width: u32, height: u32, flags: u8) -> Result<Self, ContainerError> {
        if !dimensions_fit_u24_minus_one(width, height) {
            return Err(error(
                ContainerErrorKind::InvalidDimensions,
                "container dimensions exceed the VP8X wire range",
            ));
        }
        let mut vp8x = [0_u8; 10];
        vp8x[0] = flags;
        vp8x[4..7].copy_from_slice(&(width - 1).to_le_bytes()[..3]);
        vp8x[7..10].copy_from_slice(&(height - 1).to_le_bytes()[..3]);
        let mut muxer = Self::new();
        muxer.add_chunk(MuxChunk::new(VP8X, copy_bytes(&vp8x)?))?;
        Ok(muxer)
    }

    pub(crate) fn from_chunks(chunks: Vec<MuxChunk>) -> Self {
        Self { chunks }
    }
}

#[cfg(test)]
#[path = "mux_tests.rs"]
mod tests;
