//! Forward-only RIFF and top-level chunk framing for incremental decode.

use super::CodecKind;
use crate::CompatibilityProfile;
use crate::DecodeError;
use crate::DecodeErrorKind;
use crate::DecodeLimits;
use webp_utils::read_u24_le;

const RIFF_HEADER_LEN: usize = 12;
const CHUNK_HEADER_LEN: usize = 8;
const VP8X_PAYLOAD_LEN: usize = 10;
const VP8X_ALPHA_FLAG: u8 = 0x10;

#[derive(Debug, Clone, Copy)]
pub(super) struct Vp8xState {
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) alpha: bool,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct ImageChunk {
    pub(super) offset: usize,
    pub(super) payload_start: usize,
    pub(super) payload_len: usize,
    pub(super) kind: CodecKind,
}

impl ImageChunk {
    pub(super) fn available_payload(self, bytes: &[u8]) -> &[u8] {
        if bytes.len() <= self.payload_start {
            &bytes[0..0]
        } else {
            &bytes[self.payload_start..bytes.len().min(self.payload_start + self.payload_len)]
        }
    }

    pub(super) const fn payload_complete(self, input_len: usize) -> bool {
        input_len >= self.payload_start + self.payload_len
    }

    pub(super) fn payload_range(self) -> std::ops::Range<usize> {
        self.payload_start..self.payload_start + self.payload_len
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct RawChunk {
    pub(super) offset: usize,
    fourcc: [u8; 4],
    payload_start: usize,
    payload_len: usize,
    chunk_end: usize,
}

impl RawChunk {
    fn as_image(self, kind: CodecKind) -> ImageChunk {
        ImageChunk {
            offset: self.offset,
            payload_start: self.payload_start,
            payload_len: self.payload_len,
            kind,
        }
    }

    pub(super) fn payload_range(self) -> std::ops::Range<usize> {
        self.payload_start..self.payload_start + self.payload_len
    }

    pub(super) const fn payload_complete(self, input_len: usize) -> bool {
        input_len >= self.payload_start + self.payload_len
    }
}

#[derive(Debug, Clone)]
pub(super) struct RiffState {
    riff_end: Option<usize>,
    next_chunk: usize,
    current: Option<RawChunk>,
    pub(super) vp8x: Option<Vp8xState>,
    pub(super) alpha: Option<RawChunk>,
    pub(super) image: Option<ImageChunk>,
    metadata_bytes: usize,
}

impl Default for RiffState {
    fn default() -> Self {
        Self {
            riff_end: None,
            next_chunk: RIFF_HEADER_LEN,
            current: None,
            vp8x: None,
            alpha: None,
            image: None,
            metadata_bytes: 0,
        }
    }
}

impl RiffState {
    pub(super) fn advance(
        &mut self,
        bytes: &[u8],
        compatibility: CompatibilityProfile,
        limits: &DecodeLimits,
    ) -> Result<(), DecodeError> {
        if self.riff_end.is_none() {
            if bytes.len() < RIFF_HEADER_LEN {
                return Ok(());
            }
            if bytes[..4] != *b"RIFF" || bytes[8..12] != *b"WEBP" {
                return Err(DecodeError::at(
                    DecodeErrorKind::InvalidContainer,
                    0,
                    "missing RIFF/WEBP magic",
                ));
            }
            let declared = u32::from_le_bytes(bytes[4..8].try_into().expect("four bytes"));
            if declared < 4 {
                return Err(DecodeError::at(
                    DecodeErrorKind::InvalidContainer,
                    4,
                    "RIFF size excludes WEBP form type",
                ));
            }
            let end = 8_usize.checked_add(declared as usize).ok_or_else(|| {
                DecodeError::at(DecodeErrorKind::InvalidContainer, 4, "RIFF size overflow")
            })?;
            self.riff_end = Some(end);
        }
        let riff_end = self.riff_end.expect("initialized above");
        if compatibility == CompatibilityProfile::SpecStrict && bytes.len() > riff_end {
            return Err(DecodeError::at(
                DecodeErrorKind::InvalidContainer,
                riff_end,
                "bytes trail declared RIFF body",
            ));
        }

        loop {
            if let Some(chunk) = self.current {
                self.inspect_current(bytes, chunk, limits)?;
                if bytes.len() < chunk.chunk_end {
                    break;
                }
                if compatibility == CompatibilityProfile::SpecStrict
                    && chunk.payload_len % 2 == 1
                    && bytes[chunk.chunk_end - 1] != 0
                {
                    return Err(DecodeError::at(
                        DecodeErrorKind::InvalidContainer,
                        chunk.chunk_end - 1,
                        "non-zero RIFF padding",
                    ));
                }
                self.next_chunk = chunk.chunk_end;
                self.current = None;
                continue;
            }
            if self.next_chunk == riff_end || bytes.len() < self.next_chunk + CHUNK_HEADER_LEN {
                break;
            }
            if riff_end - self.next_chunk < CHUNK_HEADER_LEN {
                return Err(DecodeError::at(
                    DecodeErrorKind::InvalidContainer,
                    self.next_chunk,
                    "truncated chunk header inside declared RIFF body",
                ));
            }
            let fourcc: [u8; 4] = bytes[self.next_chunk..self.next_chunk + 4]
                .try_into()
                .expect("four bytes");
            let payload_len = u32::from_le_bytes(
                bytes[self.next_chunk + 4..self.next_chunk + 8]
                    .try_into()
                    .expect("four bytes"),
            ) as usize;
            let chunk_end = self
                .next_chunk
                .checked_add(CHUNK_HEADER_LEN)
                .and_then(|end| end.checked_add(payload_len))
                .and_then(|end| end.checked_add(payload_len & 1))
                .ok_or_else(|| {
                    DecodeError::at(
                        DecodeErrorKind::InvalidContainer,
                        self.next_chunk,
                        "RIFF chunk end overflows",
                    )
                })?;
            if chunk_end > riff_end {
                return Err(DecodeError::at(
                    DecodeErrorKind::UnexpectedEof,
                    self.next_chunk,
                    "RIFF chunk exceeds declared body",
                ));
            }
            let chunk = RawChunk {
                offset: self.next_chunk,
                fourcc,
                payload_start: self.next_chunk + CHUNK_HEADER_LEN,
                payload_len,
                chunk_end,
            };
            if matches!(&fourcc, b"ICCP" | b"EXIF" | b"XMP ") {
                self.metadata_bytes =
                    self.metadata_bytes
                        .checked_add(payload_len)
                        .ok_or_else(|| {
                            DecodeError::at(
                                DecodeErrorKind::LimitExceeded,
                                self.next_chunk,
                                "metadata byte count overflows",
                            )
                        })?;
                if self.metadata_bytes > limits.max_metadata_bytes {
                    return Err(DecodeError::at(
                        DecodeErrorKind::LimitExceeded,
                        self.next_chunk,
                        "metadata exceeds max_metadata_bytes",
                    ));
                }
            }
            if fourcc == *b"VP8L" && self.image.is_none() {
                self.image = Some(chunk.as_image(CodecKind::Vp8l));
            } else if fourcc == *b"VP8 " && self.image.is_none() {
                self.image = Some(chunk.as_image(CodecKind::Vp8));
            } else if fourcc == *b"ALPH" && self.alpha.is_none() {
                self.alpha = Some(chunk);
            }
            self.current = Some(chunk);
        }
        Ok(())
    }

    fn inspect_current(
        &mut self,
        bytes: &[u8],
        chunk: RawChunk,
        limits: &DecodeLimits,
    ) -> Result<(), DecodeError> {
        if chunk.fourcc == *b"VP8X"
            && self.vp8x.is_none()
            && bytes.len() >= chunk.payload_start + VP8X_PAYLOAD_LEN
        {
            if chunk.payload_len != VP8X_PAYLOAD_LEN {
                return Err(DecodeError::at(
                    DecodeErrorKind::InvalidContainer,
                    chunk.offset,
                    "VP8X payload must be ten bytes",
                ));
            }
            let payload = &bytes[chunk.payload_range()];
            let flags = payload[0];
            let width = read_u24_le(payload[4..7].try_into().expect("validated VP8X width")) + 1;
            let height = read_u24_le(payload[7..10].try_into().expect("validated VP8X height")) + 1;
            limits.check_image(width, height)?;
            self.vp8x = Some(Vp8xState {
                width,
                height,
                alpha: flags & VP8X_ALPHA_FLAG != 0,
            });
        }
        Ok(())
    }

    pub(super) const fn is_container_complete(&self, input_len: usize) -> bool {
        match self.riff_end {
            Some(end) => input_len >= end && self.current.is_none() && self.next_chunk == end,
            None => false,
        }
    }
}
