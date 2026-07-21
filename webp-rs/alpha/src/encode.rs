//! Encoding primitives for WebP's `ALPH` chunk payload.

use crate::AlphaCompression;
use crate::AlphaFilter;
use crate::AlphaHeader;
use crate::AlphaPreprocessing;
use webp_core::BitWriter;

const MAX_LOSSLESS_DIMENSION: u32 = 1 << 14;
const GREEN_ALPHABET_SIZE: usize = 256 + 24;
const CHANNEL_ALPHABET_SIZE: usize = 256;

/// Configuration for encoding one complete `ALPH` payload.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AlphaEncodeOptions {
    pub compression: AlphaCompression,
    pub filter: AlphaFilter,
    /// Informative preprocessing already represented by `samples`.
    ///
    /// `LevelReduction` sets the corresponding header field; the WebP format
    /// does not prescribe a quantization algorithm, so this encoder does not
    /// alter the caller's samples.
    pub preprocessing: AlphaPreprocessing,
}

impl Default for AlphaEncodeOptions {
    fn default() -> Self {
        Self {
            compression: AlphaCompression::Raw,
            filter: AlphaFilter::None,
            preprocessing: AlphaPreprocessing::None,
        }
    }
}

/// Stable reason an alpha-plane encoding operation failed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AlphaEncodeError {
    /// Width or height is zero, or lossless dimensions exceed VP8L's limit.
    InvalidDimensions,
    /// The input is not exactly `width * height` alpha samples.
    InvalidSampleLength,
    /// Image or output byte-size arithmetic overflowed the host address space.
    SizeOverflow,
    /// Reserving output storage failed.
    AllocationFailed,
}

impl core::fmt::Display for AlphaEncodeError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidDimensions => formatter.write_str("invalid ALPH dimensions"),
            Self::InvalidSampleLength => {
                formatter.write_str("alpha sample length does not match dimensions")
            }
            Self::SizeOverflow => formatter.write_str("ALPH output size overflow"),
            Self::AllocationFailed => formatter.write_str("ALPH output allocation failed"),
        }
    }
}

impl std::error::Error for AlphaEncodeError {}

/// Encodes row-major alpha samples as a complete `ALPH` chunk payload.
///
/// The returned bytes begin with the one-byte `ALPH` header. Raw compression
/// stores the filtered samples directly. Lossless compression emits a
/// headerless VP8L image stream whose green channel contains those samples.
pub fn encode(
    samples: &[u8],
    width: u32,
    height: u32,
    options: AlphaEncodeOptions,
) -> Result<Vec<u8>, AlphaEncodeError> {
    let sample_len = validate_input(samples, width, height, options.compression)?;
    let filtered = filter(samples, sample_len, width, options.filter)?;

    let encoded_samples = match options.compression {
        AlphaCompression::Raw => filtered,
        AlphaCompression::Lossless => encode_lossless(&filtered)?,
    };
    let output_len = encoded_samples
        .len()
        .checked_add(1)
        .ok_or(AlphaEncodeError::SizeOverflow)?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(output_len)
        .map_err(|_| AlphaEncodeError::AllocationFailed)?;
    output.push(
        AlphaHeader {
            compression: options.compression,
            filter: options.filter,
            preprocessing: options.preprocessing,
        }
        .to_byte(),
    );
    output.extend_from_slice(&encoded_samples);
    Ok(output)
}

fn validate_input(
    samples: &[u8],
    width: u32,
    height: u32,
    compression: AlphaCompression,
) -> Result<usize, AlphaEncodeError> {
    if width == 0
        || height == 0
        || (compression == AlphaCompression::Lossless
            && (width > MAX_LOSSLESS_DIMENSION || height > MAX_LOSSLESS_DIMENSION))
    {
        return Err(AlphaEncodeError::InvalidDimensions);
    }
    let expected = usize::try_from(u64::from(width) * u64::from(height))
        .map_err(|_| AlphaEncodeError::SizeOverflow)?;
    if samples.len() != expected {
        return Err(AlphaEncodeError::InvalidSampleLength);
    }
    Ok(expected)
}

fn filter(
    samples: &[u8],
    sample_len: usize,
    width: u32,
    filter: AlphaFilter,
) -> Result<Vec<u8>, AlphaEncodeError> {
    let width = usize::try_from(width).map_err(|_| AlphaEncodeError::SizeOverflow)?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(sample_len)
        .map_err(|_| AlphaEncodeError::AllocationFailed)?;
    for (index, &sample) in samples.iter().enumerate() {
        let x = index % width;
        let y = index / width;
        let left = if x != 0 { samples[index - 1] } else { 0 };
        let top = if y != 0 { samples[index - width] } else { 0 };
        let top_left = if x != 0 && y != 0 {
            samples[index - width - 1]
        } else {
            0
        };
        let predictor = match filter {
            AlphaFilter::None => 0,
            AlphaFilter::Horizontal => {
                if x == 0 {
                    top
                } else {
                    left
                }
            }
            AlphaFilter::Vertical => {
                if y == 0 {
                    left
                } else {
                    top
                }
            }
            AlphaFilter::Gradient => {
                if x == 0 {
                    top
                } else if y == 0 {
                    left
                } else {
                    gradient(left, top, top_left)
                }
            }
        };
        output.push(sample.wrapping_sub(predictor));
    }
    Ok(output)
}

fn encode_lossless(samples: &[u8]) -> Result<Vec<u8>, AlphaEncodeError> {
    let mut writer = BitWriter::new();
    write_bits(&mut writer, 0, 1)?; // Transform-list terminator.
    write_bits(&mut writer, 0, 1)?; // No color cache.
    write_bits(&mut writer, 0, 1)?; // One entropy group, not meta-Huffman.
    write_literal_table(&mut writer, GREEN_ALPHABET_SIZE)?;
    write_simple_table(&mut writer, 0)?; // Red is unused.
    write_simple_table(&mut writer, 0)?; // Blue is unused.
    write_simple_table(&mut writer, u8::MAX)?; // Opaque VP8L alpha lane.
    write_simple_table(&mut writer, 0)?; // Distance codes are unused.
    for &sample in samples {
        write_canonical_symbol(&mut writer, u32::from(sample), 8)?;
    }
    Ok(writer.into_bytes())
}

fn write_literal_table(
    writer: &mut BitWriter,
    alphabet_size: usize,
) -> Result<(), AlphaEncodeError> {
    debug_assert!(CHANNEL_ALPHABET_SIZE <= alphabet_size);
    write_bits(writer, 0, 1)?; // Normal Huffman-code representation.
    write_bits(writer, 8, 4)?; // Twelve code-length-code entries.
    for length in [0_u32, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 1] {
        write_bits(writer, length, 3)?;
    }
    write_bits(writer, 0, 1)?; // No max-symbol shortening.
    for symbol in 0..alphabet_size {
        write_bits(writer, u32::from(symbol < CHANNEL_ALPHABET_SIZE), 1)?;
    }
    Ok(())
}

fn write_simple_table(writer: &mut BitWriter, symbol: u8) -> Result<(), AlphaEncodeError> {
    write_bits(writer, 1, 1)?;
    write_bits(writer, 0, 1)?;
    write_bits(writer, 1, 1)?;
    write_bits(writer, u32::from(symbol), 8)
}

fn write_canonical_symbol(
    writer: &mut BitWriter,
    canonical_code: u32,
    width: u8,
) -> Result<(), AlphaEncodeError> {
    let wire_code = canonical_code.reverse_bits() >> (u32::BITS - u32::from(width));
    write_bits(writer, wire_code, width)
}

fn write_bits(writer: &mut BitWriter, value: u32, count: u8) -> Result<(), AlphaEncodeError> {
    writer
        .write_bits(value, count)
        .map_err(|_| AlphaEncodeError::AllocationFailed)
}

#[inline]
fn gradient(left: u8, top: u8, top_left: u8) -> u8 {
    (left as i16 + top as i16 - top_left as i16).clamp(0, 255) as u8
}

#[cfg(test)]
#[path = "encode_tests.rs"]
mod tests;
