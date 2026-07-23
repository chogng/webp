//! Bounded static-image incremental decoding.
//!
//! RIFF framing is consumed once in wire order. Lossy VP8 keeps arithmetic,
//! neighbour, filter, and pixel state across pushes and publishes only rows
//! that later filtering and fancy chroma upsampling can no longer change.
//! VP8L and compressed ALPH currently begin codec work when their complete
//! RIFF chunk is available; unlike the former implementation, they are still
//! decoded during `push` and are never reparsed on every append.

use crate::DecodeError;
use crate::DecodeErrorKind;
use crate::DecodeOptions;
use crate::Image;
use crate::ImageInfo;

#[path = "incremental_riff.rs"]
mod riff;

use riff::RiffState;

#[cfg(test)]
const VP8X_ANIMATION_FLAG: u8 = 0x02;

/// Progress made while accepting another incremental input fragment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Progress {
    /// The container or codec accepted the bytes but no additional output row
    /// is stable yet.
    NeedMoreData,
    /// A larger prefix of the output is now safe to consume.
    DecodedRows { decoded_rows: u32 },
    /// The full static image is decoded and can be taken with `finish`.
    Complete,
}

/// Borrowed view of the stable RGBA prefix produced by an incremental decode.
///
/// `rgba` contains exactly `decoded_rows * width * 4` bytes. Rows outside this
/// prefix can still be changed by VP8 in-loop filtering or chroma upsampling
/// and are intentionally not exposed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IncrementalImage<'a> {
    pub width: u32,
    pub height: u32,
    pub decoded_rows: u32,
    pub rgba: &'a [u8],
}

/// Persistent decoder for one static WebP RIFF stream.
///
/// VP8 output can become available before the complete compressed payload.
/// Use [`Self::decoded`] after each successful [`Self::push`] to borrow the
/// stable RGBA row prefix. Animated containers are deliberately rejected.
#[derive(Debug, Clone)]
pub struct IncrementalDecoder {
    options: DecodeOptions,
    bytes: Vec<u8>,
    parser: RiffState,
    info: Option<ImageInfo>,
    codec: CodecState,
    terminal: TerminalState,
}

#[derive(Debug, Clone)]
enum TerminalState {
    Active,
    Complete(Image),
    Failed(DecodeError),
}

#[derive(Debug, Clone)]
enum CodecState {
    Waiting,
    Vp8(Box<[crate::vp8::IncrementalVp8Decoder]>),
    Ready { image: Image, kind: CodecKind },
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum CodecKind {
    Vp8,
    Vp8l,
}

impl IncrementalDecoder {
    /// Creates an empty decoder governed by the supplied compatibility and
    /// resource-limit policy.
    #[must_use]
    pub fn new(options: DecodeOptions) -> Self {
        Self {
            options,
            bytes: Vec::new(),
            parser: RiffState::default(),
            info: None,
            codec: CodecState::Waiting,
            terminal: TerminalState::Active,
        }
    }

    /// Accepts another contiguous input fragment and advances as far as the
    /// currently available codec bytes permit.
    ///
    /// Empty pushes are valid and never change terminal state. Once `Complete`
    /// or an error has been returned, another push is an invalid parameter.
    pub fn push(&mut self, bytes: &[u8]) -> Result<Progress, DecodeError> {
        match &self.terminal {
            TerminalState::Complete(_) => {
                return Err(DecodeError::at(
                    DecodeErrorKind::InvalidParameter,
                    self.bytes.len(),
                    "push after incremental decode completed",
                ));
            }
            TerminalState::Failed(_) => {
                return Err(DecodeError::at(
                    DecodeErrorKind::InvalidParameter,
                    self.bytes.len(),
                    "push after incremental decode failed",
                ));
            }
            TerminalState::Active => {}
        }
        let total = match self.bytes.len().checked_add(bytes.len()) {
            Some(total) => total,
            None => {
                let error = DecodeError::at(
                    DecodeErrorKind::LimitExceeded,
                    self.bytes.len(),
                    "incremental input size overflow",
                );
                self.terminal = TerminalState::Failed(error.clone());
                return Err(error);
            }
        };
        if total > self.options.limits.max_input_bytes {
            let error = DecodeError::at(
                DecodeErrorKind::LimitExceeded,
                total,
                "incremental input exceeds max_input_bytes",
            );
            self.terminal = TerminalState::Failed(error.clone());
            return Err(error);
        }
        if self.bytes.try_reserve(bytes.len()).is_err() {
            let error = DecodeError::at(
                DecodeErrorKind::AllocationFailed,
                self.bytes.len(),
                "cannot reserve incremental input",
            );
            self.terminal = TerminalState::Failed(error.clone());
            return Err(error);
        }
        self.bytes.extend_from_slice(bytes);

        let previous_rows = self.decoded_rows();
        if let Err(error) = self.drive() {
            self.terminal = TerminalState::Failed(error.clone());
            return Err(error);
        }
        if matches!(self.terminal, TerminalState::Complete(_)) {
            return Ok(Progress::Complete);
        }
        let decoded_rows = self.decoded_rows();
        if decoded_rows > previous_rows {
            Ok(Progress::DecodedRows { decoded_rows })
        } else {
            Ok(Progress::NeedMoreData)
        }
    }

    /// Returns fixed image information as soon as the relevant codec header
    /// and optional VP8X canvas are available.
    #[must_use]
    pub const fn info(&self) -> Option<ImageInfo> {
        self.info
    }

    /// Returns the immutable RGBA row prefix decoded so far.
    #[must_use]
    pub fn decoded(&self) -> Option<IncrementalImage<'_>> {
        match &self.terminal {
            TerminalState::Complete(image) => Some(image_view(image)),
            TerminalState::Failed(_) => None,
            TerminalState::Active => match &self.codec {
                CodecState::Vp8(decoders)
                    if decoders
                        .first()
                        .is_some_and(|decoder| decoder.decoded_rows() != 0) =>
                {
                    let decoder = decoders.first().expect("checked above");
                    Some(IncrementalImage {
                        width: decoder.width(),
                        height: decoder.height(),
                        decoded_rows: decoder.decoded_rows(),
                        rgba: decoder.rgba(),
                    })
                }
                CodecState::Ready { image, .. } => Some(image_view(image)),
                CodecState::Waiting | CodecState::Vp8(_) => None,
            },
        }
    }

    /// Finishes the stream. A stream shorter than its declared RIFF body is a
    /// truncation even when some rows have already been published.
    pub fn finish(self) -> Result<Image, DecodeError> {
        match self.terminal {
            TerminalState::Complete(image) => Ok(image),
            TerminalState::Failed(error) => Err(error),
            TerminalState::Active => Err(DecodeError::at(
                DecodeErrorKind::UnexpectedEof,
                self.bytes.len(),
                "incremental WebP stream ended before a complete static image",
            )),
        }
    }

    fn decoded_rows(&self) -> u32 {
        self.decoded().map_or(0, |image| image.decoded_rows)
    }

    fn drive(&mut self) -> Result<(), DecodeError> {
        self.parser.advance(
            &self.bytes,
            self.options.compatibility,
            &self.options.limits,
        )?;
        self.update_info()?;
        self.advance_codec()?;

        if self.parser.is_container_complete(self.bytes.len()) {
            let container = crate::container_adapter::parse(
                &self.bytes,
                self.options.compatibility,
                &self.options.limits,
            )?;
            if container
                .vp8x()
                .is_some_and(|header| header.flags.animation())
            {
                return Err(DecodeError::at(
                    DecodeErrorKind::UnsupportedFeature,
                    0,
                    "incremental animation decoding is not supported",
                ));
            }
            let selected = selected_codec(&container)?;
            validate_static_layout(&container, selected)?;
            drop(container);
            let image = self.take_validated_image(selected)?;
            self.bytes.clear();
            self.bytes.shrink_to_fit();
            self.terminal = TerminalState::Complete(image);
        }
        Ok(())
    }

    fn update_info(&mut self) -> Result<(), DecodeError> {
        let Some(image) = self.parser.image else {
            return Ok(());
        };
        let payload = image.available_payload(&self.bytes);
        let canvas = self.parser.vp8x.map(|header| (header.width, header.height));
        let next = match image.kind {
            CodecKind::Vp8l if payload.len() >= 5 => {
                let header = crate::vp8l::header::parse_riff_payload(
                    &payload[..5],
                    canvas,
                    &self.options.limits,
                )?;
                Some(ImageInfo {
                    width: header.width,
                    height: header.height,
                    has_alpha: header.alpha_is_used,
                    is_animated: false,
                })
            }
            CodecKind::Vp8 if payload.len() >= 10 => {
                let header = crate::vp8::parse_riff_payload_prefix(
                    &payload[..10],
                    image.payload_len,
                    canvas,
                    &self.options.limits,
                )?;
                Some(ImageInfo {
                    width: header.width,
                    height: header.height,
                    has_alpha: self.parser.vp8x.is_some_and(|header| header.alpha),
                    is_animated: false,
                })
            }
            CodecKind::Vp8 | CodecKind::Vp8l => None,
        };
        if let Some(next) = next {
            self.info = Some(next);
        }
        Ok(())
    }

    fn advance_codec(&mut self) -> Result<(), DecodeError> {
        let Some(descriptor) = self.parser.image else {
            return Ok(());
        };
        let payload = descriptor.available_payload(&self.bytes);
        match descriptor.kind {
            CodecKind::Vp8l => {
                if !descriptor.payload_complete(self.bytes.len())
                    || !matches!(self.codec, CodecState::Waiting)
                {
                    return Ok(());
                }
                let decoded = crate::vp8l::image_reader::decode_vp8l(
                    &self.bytes[descriptor.payload_range()],
                    &self.options.limits,
                )?;
                if let Some(vp8x) = self.parser.vp8x
                    && (vp8x.width != decoded.header.width || vp8x.height != decoded.header.height)
                {
                    return Err(DecodeError::at(
                        DecodeErrorKind::InvalidContainer,
                        descriptor.offset,
                        "VP8X canvas does not match VP8L dimensions",
                    ));
                }
                self.codec = CodecState::Ready {
                    image: Image {
                        width: decoded.header.width,
                        height: decoded.header.height,
                        rgba: decoded.rgba,
                    },
                    kind: CodecKind::Vp8l,
                };
            }
            CodecKind::Vp8 => {
                if matches!(self.codec, CodecState::Waiting) && payload.len() >= 10 {
                    let canvas = self.parser.vp8x.map(|header| (header.width, header.height));
                    let frame = crate::vp8::parse_riff_payload_prefix(
                        &payload[..10],
                        descriptor.payload_len,
                        canvas,
                        &self.options.limits,
                    )?;
                    let alpha = self.decode_alpha(frame.width, frame.height)?;
                    match crate::vp8::IncrementalVp8Decoder::new(
                        payload,
                        descriptor.payload_len,
                        frame,
                        alpha,
                        &self.options.limits,
                    ) {
                        Ok(decoder) => self.codec = CodecState::Vp8(box_vp8_decoder(decoder)?),
                        Err(error)
                            if error.kind() == DecodeErrorKind::UnexpectedEof
                                && !descriptor.payload_complete(self.bytes.len()) => {}
                        Err(error) => return Err(error),
                    }
                }
                if let CodecState::Vp8(decoders) = &mut self.codec {
                    let decoder = decoders
                        .first_mut()
                        .expect("VP8 state owns exactly one decoder");
                    match decoder.advance(payload) {
                        Ok(()) => {}
                        Err(error)
                            if error.kind() == DecodeErrorKind::UnexpectedEof
                                && !descriptor.payload_complete(self.bytes.len()) =>
                        {
                            return Ok(());
                        }
                        Err(error) => return Err(error),
                    }
                    if decoder.is_complete() {
                        let width = decoder.width();
                        let height = decoder.height();
                        let decoder = match std::mem::replace(&mut self.codec, CodecState::Waiting)
                        {
                            CodecState::Vp8(decoders) => decoders
                                .into_vec()
                                .pop()
                                .expect("VP8 state owns exactly one decoder"),
                            _ => unreachable!("matched VP8 codec state"),
                        };
                        self.codec = CodecState::Ready {
                            image: Image {
                                width,
                                height,
                                rgba: decoder.into_rgba(),
                            },
                            kind: CodecKind::Vp8,
                        };
                    }
                }
            }
        }
        Ok(())
    }

    fn decode_alpha(&self, width: u32, height: u32) -> Result<Option<Vec<u8>>, DecodeError> {
        let declared_alpha = self.parser.vp8x.is_some_and(|header| header.alpha);
        match self.parser.alpha {
            Some(alpha) if alpha.payload_complete(self.bytes.len()) => {
                crate::alpha::decode::decode(
                    &self.bytes[alpha.payload_range()],
                    width,
                    height,
                    self.options.compatibility,
                    &self.options.limits,
                )
                .map(Some)
            }
            Some(_) => Ok(None),
            None if declared_alpha => Err(DecodeError::at(
                DecodeErrorKind::InvalidContainer,
                0,
                "VP8X declares alpha but has no ALPH chunk before VP8",
            )),
            None => Ok(None),
        }
    }

    fn take_validated_image(&mut self, selected: CodecKind) -> Result<Image, DecodeError> {
        let state = std::mem::replace(&mut self.codec, CodecState::Waiting);
        match state {
            CodecState::Ready { image, kind } if kind == selected => Ok(image),
            CodecState::Ready { .. } | CodecState::Vp8(_) | CodecState::Waiting => {
                Err(DecodeError::at(
                    DecodeErrorKind::UnexpectedEof,
                    self.bytes.len(),
                    "static codec did not complete within its RIFF chunk",
                ))
            }
        }
    }
}

fn selected_codec(container: &webp_demux::Container<'_>) -> Result<CodecKind, DecodeError> {
    if container
        .chunks()
        .iter()
        .any(|chunk| chunk.fourcc == webp_container::VP8L)
    {
        Ok(CodecKind::Vp8l)
    } else if container
        .chunks()
        .iter()
        .any(|chunk| chunk.fourcc == webp_container::VP8)
    {
        Ok(CodecKind::Vp8)
    } else {
        Err(DecodeError::at(
            DecodeErrorKind::UnsupportedFeature,
            0,
            "incremental stream contains no supported static codec",
        ))
    }
}

fn validate_static_layout(
    container: &webp_demux::Container<'_>,
    selected: CodecKind,
) -> Result<(), DecodeError> {
    if selected != CodecKind::Vp8 {
        return Ok(());
    }
    let vp8_index = container
        .chunks()
        .iter()
        .position(|chunk| chunk.fourcc == webp_container::VP8)
        .expect("selected VP8 chunk has an index");
    let alpha = container
        .chunks()
        .iter()
        .enumerate()
        .find(|(_, chunk)| chunk.fourcc == webp_container::ALPH);
    if container.vp8x().is_some_and(|header| header.flags.alpha()) && alpha.is_none() {
        return Err(DecodeError::at(
            DecodeErrorKind::InvalidContainer,
            0,
            "VP8X declares alpha but has no ALPH chunk",
        ));
    }
    if let Some((alpha_index, alpha_chunk)) = alpha
        && alpha_index > vp8_index
    {
        return Err(DecodeError::at(
            DecodeErrorKind::InvalidContainer,
            alpha_chunk.offset,
            "ALPH chunk must precede its VP8 chunk",
        ));
    }
    Ok(())
}

fn image_view(image: &Image) -> IncrementalImage<'_> {
    IncrementalImage {
        width: image.width,
        height: image.height,
        decoded_rows: image.height,
        rgba: &image.rgba,
    }
}

fn box_vp8_decoder(
    decoder: crate::vp8::IncrementalVp8Decoder,
) -> Result<Box<[crate::vp8::IncrementalVp8Decoder]>, DecodeError> {
    let mut decoders = Vec::new();
    decoders.try_reserve_exact(1).map_err(|_| {
        DecodeError::new(
            DecodeErrorKind::AllocationFailed,
            None,
            "cannot allocate incremental VP8 state",
        )
    })?;
    decoders.push(decoder);
    Ok(decoders.into_boxed_slice())
}

#[cfg(test)]
#[path = "incremental_tests.rs"]
mod tests;
