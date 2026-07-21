//! Encoding primitives for WebP's `ALPH` chunk payload.

use crate::AlphaCompression;
use crate::AlphaFilterSelection;
use crate::AlphaHeader;
use crate::AlphaPreprocessing;
use crate::encode_filter;
use crate::encode_huffman::write_adaptive_table;
use crate::encode_huffman::write_simple_table;
use crate::encode_huffman::write_table_symbol;
use crate::level_reduction;
use webp_core::BitWriter;

const MAX_LOSSLESS_DIMENSION: u32 = 1 << 14;
const GREEN_ALPHABET_SIZE: usize = 256 + 24;
const DISTANCE_ALPHABET_SIZE: usize = 40;
const CHANNEL_ALPHABET_SIZE: usize = 256;
const MIN_MATCH_LENGTH: usize = 4;
const MAX_MATCH_LENGTH: usize = 4096;
const MATCH_HASH_SIZE: usize = 1 << 16;
const MAX_LINEAR_DISTANCE: usize = 1_048_456;
const MAX_CACHED_TOKEN_SAMPLES: usize = 4 * 1024 * 1024;

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
        level_reduction::quantize(samples, options.quality)?
    } else {
        copy_samples(samples)?
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
            Some(encode_lossless(&filtered)?)
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

fn copy_samples(samples: &[u8]) -> Result<Vec<u8>, AlphaEncodeError> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(samples.len())
        .map_err(|_| AlphaEncodeError::AllocationFailed)?;
    output.extend_from_slice(samples);
    Ok(output)
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

fn encode_lossless(samples: &[u8]) -> Result<Vec<u8>, AlphaEncodeError> {
    let mut match_heads = allocate_match_heads()?;
    let mut cached_tokens = allocate_token_cache(samples.len());
    let frequencies = collect_frequencies(samples, &mut match_heads, cached_tokens.as_mut())?;
    let mut writer = BitWriter::new();
    write_bits(&mut writer, 0, 1)?; // Transform-list terminator.
    write_bits(&mut writer, 0, 1)?; // No color cache.
    write_bits(&mut writer, 0, 1)?; // One entropy group, not meta-Huffman.
    let green = write_adaptive_table(&mut writer, &frequencies.green)?;
    write_simple_table(&mut writer, 0)?; // Red is unused.
    write_simple_table(&mut writer, 0)?; // Blue is unused.
    write_simple_table(&mut writer, u8::MAX)?; // Opaque VP8L alpha lane.
    let distance = write_adaptive_table(&mut writer, &frequencies.distance)?;
    if let Some(tokens) = cached_tokens {
        for token in tokens {
            write_entropy_token(&mut writer, &green, &distance, unpack_token(token))?;
        }
    } else {
        match_heads.fill(u32::MAX);
        walk_tokens(samples, &mut match_heads, |token| {
            write_entropy_token(&mut writer, &green, &distance, token)
        })?;
    }
    Ok(writer.into_bytes())
}

fn write_entropy_token(
    writer: &mut BitWriter,
    green: &crate::encode_huffman::EncodingTable,
    distance: &crate::encode_huffman::EncodingTable,
    token: EntropyToken,
) -> Result<(), AlphaEncodeError> {
    match token {
        EntropyToken::Literal(sample) => write_table_symbol(writer, green, usize::from(sample)),
        EntropyToken::Copy {
            length,
            distance: copy_distance,
        } => {
            let (length_prefix, length_extra) = vp8l_prefix(length, 24)?;
            write_table_symbol(writer, green, CHANNEL_ALPHABET_SIZE + length_prefix)?;
            write_bits(writer, length_extra.0, length_extra.1)?;
            let distance_code = copy_distance
                .checked_add(120)
                .ok_or(AlphaEncodeError::SizeOverflow)?;
            let (distance_prefix, distance_extra) =
                vp8l_prefix(distance_code, DISTANCE_ALPHABET_SIZE)?;
            write_table_symbol(writer, distance, distance_prefix)?;
            write_bits(writer, distance_extra.0, distance_extra.1)
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EntropyToken {
    Literal(u8),
    Copy { length: usize, distance: usize },
}

struct EntropyFrequencies {
    green: [u32; GREEN_ALPHABET_SIZE],
    distance: [u32; DISTANCE_ALPHABET_SIZE],
}

fn collect_frequencies(
    samples: &[u8],
    match_heads: &mut [u32],
    mut cached_tokens: Option<&mut Vec<u32>>,
) -> Result<EntropyFrequencies, AlphaEncodeError> {
    let mut frequencies = EntropyFrequencies {
        green: [0; GREEN_ALPHABET_SIZE],
        distance: [0; DISTANCE_ALPHABET_SIZE],
    };
    walk_tokens(samples, match_heads, |token| {
        if let Some(tokens) = cached_tokens.as_mut() {
            tokens.push(pack_token(token));
        }
        match token {
            EntropyToken::Literal(sample) => {
                increment_frequency(&mut frequencies.green, usize::from(sample))
            }
            EntropyToken::Copy { length, distance } => {
                let (length_prefix, _) = vp8l_prefix(length, 24)?;
                increment_frequency(
                    &mut frequencies.green,
                    CHANNEL_ALPHABET_SIZE + length_prefix,
                )?;
                let distance_code = distance
                    .checked_add(120)
                    .ok_or(AlphaEncodeError::SizeOverflow)?;
                let (distance_prefix, _) = vp8l_prefix(distance_code, DISTANCE_ALPHABET_SIZE)?;
                increment_frequency(&mut frequencies.distance, distance_prefix)
            }
        }
    })?;
    Ok(frequencies)
}

fn allocate_token_cache(sample_count: usize) -> Option<Vec<u32>> {
    if sample_count > MAX_CACHED_TOKEN_SAMPLES {
        return None;
    }
    let mut tokens = Vec::new();
    tokens.try_reserve_exact(sample_count).ok()?;
    Some(tokens)
}

fn pack_token(token: EntropyToken) -> u32 {
    match token {
        EntropyToken::Literal(sample) => u32::from(sample),
        EntropyToken::Copy { length, distance } => {
            debug_assert!((1..=MAX_MATCH_LENGTH).contains(&length));
            debug_assert!((1..=MAX_LINEAR_DISTANCE).contains(&distance));
            ((distance as u32) << 12) | (length as u32 - 1)
        }
    }
}

fn unpack_token(token: u32) -> EntropyToken {
    let distance = (token >> 12) as usize;
    if distance == 0 {
        EntropyToken::Literal(token as u8)
    } else {
        EntropyToken::Copy {
            length: ((token & 0x0fff) + 1) as usize,
            distance,
        }
    }
}

fn walk_tokens(
    samples: &[u8],
    heads: &mut [u32],
    mut emit: impl FnMut(EntropyToken) -> Result<(), AlphaEncodeError>,
) -> Result<(), AlphaEncodeError> {
    let mut index = 0_usize;
    while index < samples.len() {
        let mut match_length = 0_usize;
        let mut match_distance = 0_usize;
        if index + MIN_MATCH_LENGTH <= samples.len() {
            let hash = match_hash(samples, index);
            let candidate = heads[hash];
            heads[hash] = u32::try_from(index).map_err(|_| AlphaEncodeError::SizeOverflow)?;
            if candidate != u32::MAX {
                let candidate =
                    usize::try_from(candidate).map_err(|_| AlphaEncodeError::SizeOverflow)?;
                let distance = index - candidate;
                if distance <= MAX_LINEAR_DISTANCE
                    && samples[candidate..candidate + MIN_MATCH_LENGTH]
                        == samples[index..index + MIN_MATCH_LENGTH]
                {
                    let limit = MAX_MATCH_LENGTH.min(samples.len() - index);
                    match_length = MIN_MATCH_LENGTH;
                    while match_length < limit
                        && samples[index + match_length] == samples[index + match_length - distance]
                    {
                        match_length += 1;
                    }
                    match_distance = distance;
                }
            }
        }

        if match_length >= MIN_MATCH_LENGTH {
            emit(EntropyToken::Copy {
                length: match_length,
                distance: match_distance,
            })?;
            for skipped in index + 1..index + match_length {
                if skipped + MIN_MATCH_LENGTH <= samples.len() {
                    heads[match_hash(samples, skipped)] =
                        u32::try_from(skipped).map_err(|_| AlphaEncodeError::SizeOverflow)?;
                }
            }
            index += match_length;
        } else {
            emit(EntropyToken::Literal(samples[index]))?;
            index += 1;
        }
    }
    Ok(())
}

fn allocate_match_heads() -> Result<Vec<u32>, AlphaEncodeError> {
    let mut heads = Vec::new();
    heads
        .try_reserve_exact(MATCH_HASH_SIZE)
        .map_err(|_| AlphaEncodeError::AllocationFailed)?;
    heads.resize(MATCH_HASH_SIZE, u32::MAX);
    Ok(heads)
}

fn match_hash(samples: &[u8], index: usize) -> usize {
    let word = u32::from(samples[index])
        | (u32::from(samples[index + 1]) << 8)
        | (u32::from(samples[index + 2]) << 16);
    ((word.wrapping_mul(0x1e35_a7bd)) >> 16) as usize
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

fn vp8l_prefix(value: usize, prefix_count: usize) -> Result<(usize, (u32, u8)), AlphaEncodeError> {
    for prefix in 0..prefix_count {
        if prefix < 4 {
            if value == prefix + 1 {
                return Ok((prefix, (0, 0)));
            }
            continue;
        }
        let prefix = u8::try_from(prefix).map_err(|_| AlphaEncodeError::SizeOverflow)?;
        let extra_bits = (prefix - 2) >> 1;
        let offset = (2_usize + usize::from(prefix & 1)) << extra_bits;
        let minimum = offset + 1;
        let maximum = minimum
            .checked_add((1_usize << extra_bits) - 1)
            .ok_or(AlphaEncodeError::SizeOverflow)?;
        if (minimum..=maximum).contains(&value) {
            return Ok((
                usize::from(prefix),
                (
                    u32::try_from(value - minimum).map_err(|_| AlphaEncodeError::SizeOverflow)?,
                    extra_bits,
                ),
            ));
        }
    }
    Err(AlphaEncodeError::SizeOverflow)
}

#[cfg(test)]
#[path = "encode_tests.rs"]
mod tests;
