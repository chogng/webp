#![forbid(unsafe_code)]
//! VP8L lossless WebP primitives.
//!
//! This first slice deliberately validates only the fixed VP8L header.  It
//! performs no entropy decoding and never allocates pixel storage.  Callers
//! handling a RIFF file should first validate its chunk framing, then pass the
//! `VP8L` payload to [`parse_riff_payload`].

use webp_core::{BitReader, DecodeError, DecodeErrorKind, DecodeLimits, checked_chunk_end};

/// The four reversible transforms defined by the VP8L lossless bitstream.
///
/// A transform may occur at most once in a main-level image.  The values are
/// the two-bit wire values from the specification.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum TransformType {
    Predictor = 0,
    Color = 1,
    SubtractGreen = 2,
    ColorIndexing = 3,
}

impl TryFrom<u8> for TransformType {
    type Error = DecodeError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Predictor),
            1 => Ok(Self::Color),
            2 => Ok(Self::SubtractGreen),
            3 => Ok(Self::ColorIndexing),
            _ => Err(DecodeError::new(
                DecodeErrorKind::InvalidBitstream,
                None,
                "invalid VP8L transform type",
            )),
        }
    }
}

/// The dimensions of a predictor or color-transform subimage.
///
/// The subimage has one pixel per square block of the main-level coded image.
/// `block_size_bits` is the exponent after VP8L's mandatory `+ 2` adjustment.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BlockTransformDescriptor {
    /// Coded dimensions immediately before this transform is read.
    pub image_width: u32,
    pub image_height: u32,
    /// `ReadBits(3) + 2`, hence always in `2..=9`.
    pub block_size_bits: u8,
    /// Width of the transform subimage, rounded up by block size.
    pub transform_width: u32,
    /// Height of the transform subimage, rounded up by block size.
    pub transform_height: u32,
}

impl BlockTransformDescriptor {
    /// Width and height of one square transform block in source pixels.
    #[must_use]
    pub const fn block_size(self) -> u32 {
        1 << self.block_size_bits
    }
}

/// Metadata needed to decode and invert a VP8L color-indexing transform.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ColorIndexingDescriptor {
    /// Coded width before palette-index packing is applied.
    pub image_width_before: u32,
    /// Coded height, which palette indexing never changes.
    pub image_height: u32,
    /// Number of ARGB entries in the separate, one-row color-table image.
    pub color_table_size: u16,
    /// `0..=3`: the number of width bits removed by palette index packing.
    pub width_bits: u8,
    /// Coded width after `ceil(image_width_before / 2^width_bits)`.
    pub image_width_after: u32,
}

impl ColorIndexingDescriptor {
    /// Width of the color-table subimage.
    #[must_use]
    pub const fn color_table_width(self) -> u32 {
        self.color_table_size as u32
    }

    /// The color table is always a single row.
    #[must_use]
    pub const fn color_table_height(self) -> u32 {
        1
    }
}

/// One VP8L transform descriptor, in the order in which it appeared on wire.
///
/// Inverse transforms must be applied by the caller in reverse order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransformDescriptor {
    Predictor(BlockTransformDescriptor),
    Color(BlockTransformDescriptor),
    SubtractGreen,
    ColorIndexing(ColorIndexingDescriptor),
}

impl TransformDescriptor {
    /// The transform's two-bit VP8L type.
    #[must_use]
    pub const fn transform_type(self) -> TransformType {
        match self {
            Self::Predictor(_) => TransformType::Predictor,
            Self::Color(_) => TransformType::Color,
            Self::SubtractGreen => TransformType::SubtractGreen,
            Self::ColorIndexing(_) => TransformType::ColorIndexing,
        }
    }
}

/// Stateful parser for VP8L's transform list.
///
/// Predictor, color, and color-indexing descriptors are followed by an image
/// encoded with VP8L image coding.  Those subimages have no transform-list
/// terminator of their own, so a caller must decode the subimage immediately
/// after [`Self::read_next`] returns its descriptor and only then call this
/// method again.  This stateful API prevents a descriptor parser from trying
/// to guess where entropy-coded data ends.
#[derive(Clone, Debug)]
pub struct TransformListParser {
    image_width: u32,
    image_height: u32,
    seen_types: u8,
    finished: bool,
}

impl TransformListParser {
    /// Starts parsing a main-level VP8L transform list.
    ///
    /// The supplied dimensions are checked before any nested transform image
    /// may be decoded.  They normally come directly from [`Vp8lHeader`].
    pub fn new(
        image_width: u32,
        image_height: u32,
        limits: &DecodeLimits,
    ) -> Result<Self, DecodeError> {
        if image_width == 0 || image_height == 0 {
            return Err(DecodeError::new(
                DecodeErrorKind::InvalidParameter,
                None,
                "VP8L transform list requires nonzero image dimensions",
            ));
        }
        limits.check_image(image_width, image_height)?;
        Ok(Self {
            image_width,
            image_height,
            seen_types: 0,
            finished: false,
        })
    }

    /// Current coded dimensions, after all previously read descriptors.
    #[must_use]
    pub const fn image_dimensions(&self) -> (u32, u32) {
        (self.image_width, self.image_height)
    }

    /// Reads the next transform descriptor or the list's terminating zero bit.
    ///
    /// A second call after the terminator is an invalid parser use rather than
    /// an attempt to consume unrelated entropy data.
    pub fn read_next(
        &mut self,
        bits: &mut BitReader<'_>,
        limits: &DecodeLimits,
    ) -> Result<Option<TransformDescriptor>, DecodeError> {
        if self.finished {
            return Err(DecodeError::at(
                DecodeErrorKind::InvalidParameter,
                bits.bit_position() / 8,
                "VP8L transform list has already ended",
            ));
        }

        if !bits.read_bit()? {
            self.finished = true;
            return Ok(None);
        }

        let type_offset = bits.bit_position() / 8;
        let transform_type = TransformType::try_from(bits.read_bits(2)? as u8).map_err(|_| {
            DecodeError::at(
                DecodeErrorKind::InvalidBitstream,
                type_offset,
                "invalid VP8L transform type",
            )
        })?;
        let type_bit = 1_u8 << (transform_type as u8);
        if self.seen_types & type_bit != 0 {
            return Err(DecodeError::at(
                DecodeErrorKind::InvalidBitstream,
                type_offset,
                "duplicate VP8L transform type",
            ));
        }
        self.seen_types |= type_bit;

        let descriptor = match transform_type {
            TransformType::Predictor => {
                TransformDescriptor::Predictor(self.read_block_descriptor(bits, limits)?)
            }
            TransformType::Color => {
                TransformDescriptor::Color(self.read_block_descriptor(bits, limits)?)
            }
            TransformType::SubtractGreen => TransformDescriptor::SubtractGreen,
            TransformType::ColorIndexing => TransformDescriptor::ColorIndexing(
                self.read_color_indexing_descriptor(bits, limits)?,
            ),
        };
        Ok(Some(descriptor))
    }

    fn read_block_descriptor(
        &self,
        bits: &mut BitReader<'_>,
        limits: &DecodeLimits,
    ) -> Result<BlockTransformDescriptor, DecodeError> {
        let block_size_bits = bits.read_bits(3)? as u8 + 2;
        let block_size = 1_u32 << block_size_bits;
        let transform_width = div_round_up(self.image_width, block_size);
        let transform_height = div_round_up(self.image_height, block_size);
        limits.check_image(transform_width, transform_height)?;
        Ok(BlockTransformDescriptor {
            image_width: self.image_width,
            image_height: self.image_height,
            block_size_bits,
            transform_width,
            transform_height,
        })
    }

    fn read_color_indexing_descriptor(
        &mut self,
        bits: &mut BitReader<'_>,
        limits: &DecodeLimits,
    ) -> Result<ColorIndexingDescriptor, DecodeError> {
        let color_table_size = bits.read_bits(8)? as u16 + 1;
        let width_bits = color_index_width_bits(color_table_size);
        // The palette itself is a separately coded 1-row subimage.  Check it
        // here so a restrictive caller limit rejects before its future decode
        // can reserve a pixel buffer.
        limits.check_image(u32::from(color_table_size), 1)?;
        let image_width_before = self.image_width;
        let image_width_after = div_round_up(image_width_before, 1 << width_bits);
        self.image_width = image_width_after;
        Ok(ColorIndexingDescriptor {
            image_width_before,
            image_height: self.image_height,
            color_table_size,
            width_bits,
            image_width_after,
        })
    }
}

/// Returns VP8L's width-subsampling exponent for a color table size.
#[must_use]
pub const fn color_index_width_bits(color_table_size: u16) -> u8 {
    match color_table_size {
        1..=2 => 3,
        3..=4 => 2,
        5..=16 => 1,
        _ => 0,
    }
}

/// Computes `ceil(value / divisor)` without an addition that can overflow.
const fn div_round_up(value: u32, divisor: u32) -> u32 {
    let quotient = value / divisor;
    if value.is_multiple_of(divisor) {
        quotient
    } else {
        quotient + 1
    }
}

/// The byte value at the beginning of every VP8L bitstream.
pub const SIGNATURE: u8 = 0x2f;

/// The number of bytes occupied by the fixed VP8L header.
pub const HEADER_LEN: usize = 5;

/// The number of bytes in a RIFF chunk header.
pub const RIFF_CHUNK_HEADER_LEN: usize = 8;

/// The largest dimension representable by the VP8L 14-bit, minus-one field.
pub const MAX_DIMENSION: u32 = 1 << 14;

/// Fixed information at the beginning of a VP8L bitstream.
///
/// `alpha_is_used` is a coding hint only.  A decoder must derive actual alpha
/// values from the later pixel data rather than treating this flag as a pixel
/// semantic.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Vp8lHeader {
    pub width: u32,
    pub height: u32,
    pub alpha_is_used: bool,
    pub version: u8,
}

/// Parses a standalone VP8L bitstream header.
///
/// `data` begins with the VP8L signature byte, not a RIFF chunk header.  The
/// function accepts bytes following the fixed header because they belong to
/// the VP8L entropy stream, which is parsed by later decoder stages.
///
/// No pixel or table allocation is attempted before [`DecodeLimits`] are
/// applied.
pub fn parse_header(data: &[u8], limits: &DecodeLimits) -> Result<Vp8lHeader, DecodeError> {
    limits.check_input_len(data.len())?;

    let mut bits = BitReader::new(data);
    let signature = bits.read_bits(8)? as u8;
    if signature != SIGNATURE {
        return Err(DecodeError::at(
            DecodeErrorKind::InvalidBitstream,
            0,
            "invalid VP8L signature",
        ));
    }

    // VP8L bits are least-significant-bit first.  The reader makes the first
    // input bit bit 0 of the returned value, exactly matching ReadBits(n).
    let width = bits.read_bits(14)? + 1;
    let height = bits.read_bits(14)? + 1;
    let alpha_is_used = bits.read_bit()?;
    let version = bits.read_bits(3)? as u8;
    if version != 0 {
        return Err(DecodeError::at(
            DecodeErrorKind::InvalidBitstream,
            4,
            "unsupported VP8L version",
        ));
    }

    // This happens before any future entropy table or pixel allocation.
    limits.check_image(width, height)?;

    Ok(Vp8lHeader {
        width,
        height,
        alpha_is_used,
        version,
    })
}

/// Parses a `VP8L` RIFF chunk payload and checks its optional `VP8X` canvas.
///
/// The caller must pass the exact payload bytes after RIFF chunk framing has
/// been validated.  `canvas` is `Some` only for an extended (`VP8X`) static
/// layout; its dimensions must exactly match the embedded VP8L header.
pub fn parse_riff_payload(
    payload: &[u8],
    canvas: Option<(u32, u32)>,
    limits: &DecodeLimits,
) -> Result<Vp8lHeader, DecodeError> {
    let header = parse_header(payload, limits)?;
    if let Some((canvas_width, canvas_height)) = canvas
        && (header.width != canvas_width || header.height != canvas_height)
    {
        return Err(DecodeError::at(
            DecodeErrorKind::InvalidContainer,
            0,
            "VP8X canvas does not match VP8L dimensions",
        ));
    }
    Ok(header)
}

/// Parses one complete, RIFF-framed `VP8L` chunk.
///
/// `chunk` starts with the four-byte `VP8L` FourCC and includes its little
/// endian length field, payload, and any required RIFF padding byte.  This is
/// useful for callers which have isolated a chunk but still need to verify its
/// declared length before handing its payload to the VP8L decoder.
pub fn parse_riff_chunk(
    chunk: &[u8],
    canvas: Option<(u32, u32)>,
    limits: &DecodeLimits,
) -> Result<Vp8lHeader, DecodeError> {
    limits.check_input_len(chunk.len())?;
    if chunk.len() < RIFF_CHUNK_HEADER_LEN {
        return Err(DecodeError::at(
            DecodeErrorKind::UnexpectedEof,
            chunk.len(),
            "truncated VP8L RIFF chunk header",
        ));
    }
    if chunk[..4] != *b"VP8L" {
        return Err(DecodeError::at(
            DecodeErrorKind::InvalidContainer,
            0,
            "expected VP8L RIFF chunk",
        ));
    }
    let payload_len = u32::from_le_bytes(chunk[4..8].try_into().map_err(|_| {
        DecodeError::at(
            DecodeErrorKind::UnexpectedEof,
            4,
            "truncated VP8L RIFF chunk length",
        )
    })?);
    let chunk_end = checked_chunk_end(0, payload_len, chunk.len())?;
    if chunk_end != chunk.len() {
        return Err(DecodeError::at(
            DecodeErrorKind::InvalidContainer,
            chunk_end,
            "VP8L RIFF chunk has trailing bytes",
        ));
    }
    let payload_len = usize::try_from(payload_len).map_err(|_| {
        DecodeError::at(
            DecodeErrorKind::InvalidContainer,
            4,
            "VP8L RIFF chunk length does not fit usize",
        )
    })?;
    let payload = &chunk[RIFF_CHUNK_HEADER_LEN..RIFF_CHUNK_HEADER_LEN + payload_len];
    parse_riff_payload(payload, canvas, limits)
}

#[cfg(test)]
mod tests {
    use super::*;
    use webp_core::BitWriter;

    fn limits() -> DecodeLimits {
        DecodeLimits::default()
    }

    fn header(width: u32, height: u32, alpha: bool, version: u8) -> Vec<u8> {
        assert!((1..=MAX_DIMENSION).contains(&width));
        assert!((1..=MAX_DIMENSION).contains(&height));
        assert!(version < 8);
        let mut writer = BitWriter::new();
        writer.write_bits(u32::from(SIGNATURE), 8).unwrap();
        writer.write_bits(width - 1, 14).unwrap();
        writer.write_bits(height - 1, 14).unwrap();
        writer.write_bits(u32::from(alpha), 1).unwrap();
        writer.write_bits(u32::from(version), 3).unwrap();
        assert_eq!(writer.as_bytes().len(), HEADER_LEN);
        writer.into_bytes()
    }

    #[test]
    fn parses_lsb_header_fields() {
        let bytes = header(0x1234, 0x0234, true, 0);
        assert_eq!(
            parse_header(&bytes, &limits()).unwrap(),
            Vp8lHeader {
                width: 0x1234,
                height: 0x0234,
                alpha_is_used: true,
                version: 0,
            }
        );
    }

    #[test]
    fn accepts_minimum_and_syntax_maximum_dimensions() {
        assert_eq!(
            parse_header(&header(1, 1, false, 0), &limits()).unwrap(),
            Vp8lHeader {
                width: 1,
                height: 1,
                alpha_is_used: false,
                version: 0,
            }
        );
        let max = parse_header(&header(MAX_DIMENSION, MAX_DIMENSION, true, 0), &limits()).unwrap();
        assert_eq!((max.width, max.height), (MAX_DIMENSION, MAX_DIMENSION));
    }

    #[test]
    fn every_signature_bit_is_checked() {
        for bit in 0..8 {
            let mut bytes = header(1, 1, false, 0);
            bytes[0] ^= 1 << bit;
            let error = parse_header(&bytes, &limits()).unwrap_err();
            assert_eq!(error.kind(), DecodeErrorKind::InvalidBitstream);
            assert_eq!(error.offset(), Some(0));
        }
    }

    #[test]
    fn rejects_nonzero_version() {
        for version in 1..8 {
            let error = parse_header(&header(1, 1, false, version), &limits()).unwrap_err();
            assert_eq!(error.kind(), DecodeErrorKind::InvalidBitstream);
            assert_eq!(error.offset(), Some(4));
        }
    }

    #[test]
    fn every_header_truncation_is_eof() {
        let bytes = header(42, 19, false, 0);
        for length in 0..HEADER_LEN {
            let error = parse_header(&bytes[..length], &limits()).unwrap_err();
            assert_eq!(
                error.kind(),
                DecodeErrorKind::UnexpectedEof,
                "length {length}"
            );
        }
    }

    #[test]
    fn limits_apply_before_any_pixel_allocation() {
        let configured = DecodeLimits {
            max_width: MAX_DIMENSION,
            max_height: MAX_DIMENSION,
            max_pixels: 3,
            ..DecodeLimits::default()
        };
        let error = parse_header(&header(2, 2, false, 0), &configured).unwrap_err();
        assert_eq!(error.kind(), DecodeErrorKind::LimitExceeded);
    }

    #[test]
    fn input_byte_limit_is_checked_before_reading() {
        let configured = DecodeLimits {
            max_input_bytes: HEADER_LEN - 1,
            ..DecodeLimits::default()
        };
        let error = parse_header(&header(1, 1, false, 0), &configured).unwrap_err();
        assert_eq!(error.kind(), DecodeErrorKind::LimitExceeded);
    }

    #[test]
    fn riff_payload_validates_canvas_consistency() {
        let bytes = header(7, 9, false, 0);
        assert_eq!(
            parse_riff_payload(&bytes, Some((7, 9)), &limits())
                .unwrap()
                .width,
            7
        );
        let error = parse_riff_payload(&bytes, Some((8, 9)), &limits()).unwrap_err();
        assert_eq!(error.kind(), DecodeErrorKind::InvalidContainer);
    }

    #[test]
    fn riff_chunk_validates_declared_payload_length() {
        let payload = header(7, 9, false, 0);
        let mut chunk = b"VP8L".to_vec();
        chunk.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        chunk.extend_from_slice(&payload);
        chunk.push(0); // VP8L header length is odd, so RIFF requires padding.
        assert_eq!(parse_riff_chunk(&chunk, None, &limits()).unwrap().height, 9);

        let mut truncated = chunk.clone();
        truncated.pop();
        assert_eq!(
            parse_riff_chunk(&truncated, None, &limits())
                .unwrap_err()
                .kind(),
            DecodeErrorKind::UnexpectedEof
        );

        chunk.push(0);
        assert_eq!(
            parse_riff_chunk(&chunk, None, &limits())
                .unwrap_err()
                .kind(),
            DecodeErrorKind::InvalidContainer
        );
    }

    fn transform_parser(width: u32, height: u32) -> TransformListParser {
        TransformListParser::new(width, height, &limits()).unwrap()
    }

    #[test]
    fn transform_descriptors_round_up_their_subimages() {
        let mut writer = BitWriter::new();
        writer.write_bits(1, 1).unwrap(); // transform present
        writer.write_bits(0, 2).unwrap(); // predictor
        writer.write_bits(3, 3).unwrap(); // block_size_bits = 5 (32 pixels)
        writer.write_bits(0, 1).unwrap(); // transform list terminator

        let mut bits = BitReader::new(writer.as_bytes());
        let mut parser = transform_parser(33, 65);
        let descriptor = parser.read_next(&mut bits, &limits()).unwrap().unwrap();
        assert_eq!(
            descriptor,
            TransformDescriptor::Predictor(BlockTransformDescriptor {
                image_width: 33,
                image_height: 65,
                block_size_bits: 5,
                transform_width: 2,
                transform_height: 3,
            })
        );
        assert_eq!(parser.read_next(&mut bits, &limits()).unwrap(), None);
        assert_eq!(parser.image_dimensions(), (33, 65));
    }

    #[test]
    fn palette_packing_changes_dimensions_for_later_descriptors() {
        let mut writer = BitWriter::new();
        writer.write_bits(1, 1).unwrap(); // transform present
        writer.write_bits(3, 2).unwrap(); // color indexing
        writer.write_bits(1, 8).unwrap(); // color_table_size = 2, width_bits = 3
        writer.write_bits(1, 1).unwrap(); // transform present
        writer.write_bits(1, 2).unwrap(); // color transform
        writer.write_bits(0, 3).unwrap(); // block_size_bits = 2 (4 pixels)
        writer.write_bits(0, 1).unwrap(); // transform list terminator

        let mut bits = BitReader::new(writer.as_bytes());
        let mut parser = transform_parser(17, 9);
        assert_eq!(
            parser.read_next(&mut bits, &limits()).unwrap(),
            Some(TransformDescriptor::ColorIndexing(
                ColorIndexingDescriptor {
                    image_width_before: 17,
                    image_height: 9,
                    color_table_size: 2,
                    width_bits: 3,
                    image_width_after: 3,
                }
            ))
        );
        assert_eq!(parser.image_dimensions(), (3, 9));
        assert_eq!(
            parser.read_next(&mut bits, &limits()).unwrap(),
            Some(TransformDescriptor::Color(BlockTransformDescriptor {
                image_width: 3,
                image_height: 9,
                block_size_bits: 2,
                transform_width: 1,
                transform_height: 3,
            }))
        );
        assert_eq!(parser.read_next(&mut bits, &limits()).unwrap(), None);
    }

    #[test]
    fn each_transform_type_can_only_appear_once() {
        let mut writer = BitWriter::new();
        writer.write_bits(1, 1).unwrap(); // transform present
        writer.write_bits(2, 2).unwrap(); // subtract green
        writer.write_bits(1, 1).unwrap(); // transform present
        writer.write_bits(2, 2).unwrap(); // duplicate subtract green

        let mut bits = BitReader::new(writer.as_bytes());
        let mut parser = transform_parser(1, 1);
        assert_eq!(
            parser.read_next(&mut bits, &limits()).unwrap(),
            Some(TransformDescriptor::SubtractGreen)
        );
        let error = parser.read_next(&mut bits, &limits()).unwrap_err();
        assert_eq!(error.kind(), DecodeErrorKind::InvalidBitstream);
    }

    #[test]
    fn descriptor_truncation_reports_eof_without_advancing_past_input() {
        let mut parser = transform_parser(1, 1);
        let mut empty = BitReader::new(&[]);
        assert_eq!(
            parser.read_next(&mut empty, &limits()).unwrap_err().kind(),
            DecodeErrorKind::UnexpectedEof
        );

        // The present flag and transform type fit in this byte, but a color
        // table descriptor must then read eight more bits.
        let mut truncated_color_table = BitReader::new(&[0b0000_0111]);
        let mut parser = transform_parser(1, 1);
        assert_eq!(
            parser
                .read_next(&mut truncated_color_table, &limits())
                .unwrap_err()
                .kind(),
            DecodeErrorKind::UnexpectedEof
        );
    }

    #[test]
    fn palette_dimensions_and_main_dimensions_honor_limits() {
        let constrained = DecodeLimits {
            max_width: 16,
            max_height: 16,
            max_pixels: 256,
            ..DecodeLimits::default()
        };
        assert_eq!(
            TransformListParser::new(17, 1, &constrained)
                .unwrap_err()
                .kind(),
            DecodeErrorKind::LimitExceeded
        );

        let mut writer = BitWriter::new();
        writer.write_bits(1, 1).unwrap(); // transform present
        writer.write_bits(3, 2).unwrap(); // color indexing
        writer.write_bits(255, 8).unwrap(); // color_table_size = 256
        let mut bits = BitReader::new(writer.as_bytes());
        let mut parser = TransformListParser::new(1, 1, &constrained).unwrap();
        assert_eq!(
            parser
                .read_next(&mut bits, &constrained)
                .unwrap_err()
                .kind(),
            DecodeErrorKind::LimitExceeded
        );

        assert_eq!(
            TransformListParser::new(0, 1, &limits())
                .unwrap_err()
                .kind(),
            DecodeErrorKind::InvalidParameter
        );
    }

    #[test]
    fn palette_width_bits_follow_all_spec_ranges() {
        assert_eq!(color_index_width_bits(1), 3);
        assert_eq!(color_index_width_bits(2), 3);
        assert_eq!(color_index_width_bits(3), 2);
        assert_eq!(color_index_width_bits(4), 2);
        assert_eq!(color_index_width_bits(5), 1);
        assert_eq!(color_index_width_bits(16), 1);
        assert_eq!(color_index_width_bits(17), 0);
        assert_eq!(color_index_width_bits(256), 0);
    }
}
