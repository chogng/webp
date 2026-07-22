//! Writes complete WebP `ALPH` chunk payloads.

mod backward_references;
mod encode_token_output;
mod filters;
mod level_reduction;
mod palette_plan;
mod symbol_plan;

#[cfg(feature = "alpha-benchmark-internals")]
pub use encode_token_output::BenchmarkWriterVariant;
#[cfg(feature = "alpha-benchmark-internals")]
pub use encode_token_output::set_benchmark_writer_variant;
pub use filters::AlphaFilterSelection;

use crate::vp8l::huffman::WireWriteError;
use crate::vp8l::huffman::write_simple_table;
use std::borrow::Cow;
use webp_utils::BitWriter;

use self::backward_references as encode_lz77;
use self::backward_references::Token;
use self::filters as encode_filter;
use self::palette_plan as encode_palette;
use self::symbol_plan::write_adaptive_table;
use webp_container::AlphaCompression;
use webp_container::AlphaHeader;
use webp_container::AlphaPreprocessing;

const MAX_LOSSLESS_DIMENSION: u32 = 1 << 14;
/// Configuration for encoding one complete `ALPH` payload.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AlphaEncodeOptions {
    pub compression: AlphaCompression,
    pub filter: AlphaFilterSelection,
    /// Alpha quality from 0 through 100. Values below 100 apply level
    /// reduction and mark the payload as preprocessed.
    pub quality: u8,
}

impl Default for AlphaEncodeOptions {
    fn default() -> Self {
        Self {
            compression: AlphaCompression::Lossless,
            filter: AlphaFilterSelection::Fast,
            quality: 100,
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
    /// Alpha quality is outside the supported 0 through 100 range.
    InvalidQuality,
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
            Self::InvalidQuality => formatter.write_str("alpha quality must be in 0 through 100"),
        }
    }
}

impl std::error::Error for AlphaEncodeError {}

impl From<WireWriteError> for AlphaEncodeError {
    fn from(error: WireWriteError) -> Self {
        match error {
            WireWriteError::SizeOverflow => Self::SizeOverflow,
            WireWriteError::AllocationFailed => Self::AllocationFailed,
        }
    }
}

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
    let (width, height) = validate_input(samples, width, height, options)?;
    let preprocessed = if options.quality < 100 {
        Cow::Owned(level_reduction::quantize(samples, options.quality)?)
    } else {
        Cow::Borrowed(samples)
    };
    let preprocessing = if options.quality < 100 {
        AlphaPreprocessing::LevelReduction
    } else {
        AlphaPreprocessing::None
    };
    let selection = if options.compression == AlphaCompression::Raw {
        AlphaFilterSelection::default()
    } else {
        options.filter
    };

    let mut best: Option<Vec<u8>> = None;
    for filter in encode_filter::candidates(&preprocessed, width, height, selection) {
        let filtered = encode_filter::apply(&preprocessed, width, filter)?;
        let lossless = if options.compression == AlphaCompression::Lossless {
            Some(encode_lossless(&filtered, width)?)
        } else {
            None
        };
        let (compression, encoded_samples) = match lossless {
            Some(encoded) if encoded.len() <= filtered.len() => {
                (AlphaCompression::Lossless, encoded)
            }
            _ => (AlphaCompression::Raw, filtered),
        };
        let candidate = make_payload(
            encoded_samples,
            AlphaHeader {
                compression,
                filter,
                preprocessing,
            },
        )?;
        if best
            .as_ref()
            .is_none_or(|current| candidate.len() < current.len())
        {
            best = Some(candidate);
        }
    }
    best.ok_or(AlphaEncodeError::InvalidDimensions)
}

fn validate_input(
    samples: &[u8],
    width: u32,
    height: u32,
    options: AlphaEncodeOptions,
) -> Result<(usize, usize), AlphaEncodeError> {
    if options.quality > 100 {
        return Err(AlphaEncodeError::InvalidQuality);
    }
    if width == 0
        || height == 0
        || (options.compression == AlphaCompression::Lossless
            && (width > MAX_LOSSLESS_DIMENSION || height > MAX_LOSSLESS_DIMENSION))
    {
        return Err(AlphaEncodeError::InvalidDimensions);
    }
    let expected = usize::try_from(u64::from(width) * u64::from(height))
        .map_err(|_| AlphaEncodeError::SizeOverflow)?;
    if samples.len() != expected {
        return Err(AlphaEncodeError::InvalidSampleLength);
    }
    let width = usize::try_from(width).map_err(|_| AlphaEncodeError::SizeOverflow)?;
    let height = usize::try_from(height).map_err(|_| AlphaEncodeError::SizeOverflow)?;
    Ok((width, height))
}

fn make_payload(
    encoded_samples: Vec<u8>,
    header: AlphaHeader,
) -> Result<Vec<u8>, AlphaEncodeError> {
    let output_len = encoded_samples
        .len()
        .checked_add(1)
        .ok_or(AlphaEncodeError::SizeOverflow)?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(output_len)
        .map_err(|_| AlphaEncodeError::AllocationFailed)?;
    output.push(header.to_byte());
    output.extend_from_slice(&encoded_samples);
    Ok(output)
}

fn encode_lossless(samples: &[u8], width: usize) -> Result<Vec<u8>, AlphaEncodeError> {
    if let Some(palette) = encode_palette::make_plan(samples, width)? {
        if palette.entries.len() == 1 {
            return encode_plain_lossless(samples, width);
        }
        let indexed = encode_palette_lossless(&palette)?;
        if samples.len() >= 1024 {
            return Ok(indexed);
        }
        let plain = encode_plain_lossless(samples, width)?;
        return Ok(if indexed.len() < plain.len() {
            indexed
        } else {
            plain
        });
    }
    encode_plain_lossless(samples, width)
}

fn encode_plain_lossless(samples: &[u8], width: usize) -> Result<Vec<u8>, AlphaEncodeError> {
    let mut writer = BitWriter::new();
    write_bits(&mut writer, 0, 1)?; // Transform-list terminator.
    encode_entropy_stream(samples, width, writer)
}

fn encode_palette_lossless(
    palette: &encode_palette::PalettePlan,
) -> Result<Vec<u8>, AlphaEncodeError> {
    let mut writer = BitWriter::new();
    encode_palette::write_transform(&mut writer, &palette.entries)?;
    encode_entropy_stream(&palette.indexed_samples, palette.indexed_width, writer)
}

fn encode_entropy_stream(
    samples: &[u8],
    width: usize,
    mut writer: BitWriter,
) -> Result<Vec<u8>, AlphaEncodeError> {
    let mut match_table = encode_lz77::MatchTable::allocate(samples.len())
        .map_err(|_| AlphaEncodeError::AllocationFailed)?;
    let mut cached_tokens = encode_lz77::allocate_token_cache(samples.len());
    let frequencies =
        collect_frequencies(samples, width, &mut match_table, cached_tokens.as_mut())?;
    write_bits(&mut writer, 0, 1)?; // No color cache.
    write_bits(&mut writer, 0, 1)?; // One entropy group, not meta-Huffman.
    let green = write_adaptive_table(&mut writer, &frequencies.green)?;
    write_simple_table(&mut writer, 0)?; // Red is unused.
    write_simple_table(&mut writer, 0)?; // Blue is unused.
    write_simple_table(&mut writer, u8::MAX)?; // Opaque VP8L alpha lane.
    let distance = write_adaptive_table(&mut writer, &frequencies.distance)?;
    encode_token_output::write_tokens(
        samples,
        width,
        &mut match_table,
        cached_tokens.as_deref(),
        writer,
        &green,
        &distance,
    )
}

struct EntropyFrequencies {
    green: [u32; encode_lz77::GREEN_ALPHABET_SIZE],
    distance: [u32; encode_lz77::DISTANCE_ALPHABET_SIZE],
}

fn collect_frequencies(
    samples: &[u8],
    width: usize,
    match_table: &mut encode_lz77::MatchTable,
    mut cached_tokens: Option<&mut Vec<u32>>,
) -> Result<EntropyFrequencies, AlphaEncodeError> {
    let mut frequencies = EntropyFrequencies {
        green: [0; encode_lz77::GREEN_ALPHABET_SIZE],
        distance: [0; encode_lz77::DISTANCE_ALPHABET_SIZE],
    };
    encode_lz77::walk(samples, match_table, |token| {
        if let Some(tokens) = cached_tokens.as_mut() {
            tokens.push(encode_lz77::pack(token));
        }
        match token {
            Token::Literal(sample) => {
                increment_frequency(&mut frequencies.green, usize::from(sample))
            }
            Token::Copy { length, distance } => {
                let length_prefix =
                    encode_lz77::prefix_code(length, encode_lz77::LENGTH_PREFIX_COUNT)
                        .ok_or(AlphaEncodeError::SizeOverflow)?;
                increment_frequency(
                    &mut frequencies.green,
                    encode_lz77::CHANNEL_ALPHABET_SIZE + length_prefix.symbol,
                )?;
                let distance_code = encode_lz77::distance_code(width, distance);
                let distance_prefix =
                    encode_lz77::prefix_code(distance_code, encode_lz77::DISTANCE_ALPHABET_SIZE)
                        .ok_or(AlphaEncodeError::SizeOverflow)?;
                increment_frequency(&mut frequencies.distance, distance_prefix.symbol)
            }
        }
    })?;
    Ok(frequencies)
}

fn increment_frequency(table: &mut [u32], symbol: usize) -> Result<(), AlphaEncodeError> {
    let frequency = table
        .get_mut(symbol)
        .ok_or(AlphaEncodeError::SizeOverflow)?;
    *frequency = frequency
        .checked_add(1)
        .ok_or(AlphaEncodeError::SizeOverflow)?;
    Ok(())
}

fn write_bits(writer: &mut BitWriter, value: u32, count: u8) -> Result<(), AlphaEncodeError> {
    writer
        .write_bits(value, count)
        .map_err(|_| AlphaEncodeError::AllocationFailed)
}

#[cfg(test)]
#[path = "plane_writer_tests.rs"]
mod tests;
