//! Encoding primitives for WebP's `ALPH` chunk payload.

use crate::AlphaCompression;
use crate::AlphaFilterSelection;
use crate::AlphaHeader;
use crate::AlphaPreprocessing;
use crate::encode_filter;
use crate::level_reduction;
use webp_core::BitWriter;

const MAX_LOSSLESS_DIMENSION: u32 = 1 << 14;
const GREEN_ALPHABET_SIZE: usize = 256 + 24;
const CODE_LENGTH_CODE_ORDER: [usize; 19] = [
    17, 18, 0, 1, 2, 3, 4, 5, 16, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15,
];

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
    let mut writer = BitWriter::new();
    write_bits(&mut writer, 0, 1)?; // Transform-list terminator.
    write_bits(&mut writer, 0, 1)?; // No color cache.
    write_bits(&mut writer, 0, 1)?; // One entropy group, not meta-Huffman.
    let mut frequencies = [0_u32; GREEN_ALPHABET_SIZE];
    for &sample in samples {
        frequencies[usize::from(sample)] = frequencies[usize::from(sample)].saturating_add(1);
    }
    let table = write_adaptive_table(&mut writer, &frequencies)?;
    write_simple_table(&mut writer, 0)?; // Red is unused.
    write_simple_table(&mut writer, 0)?; // Blue is unused.
    write_simple_table(&mut writer, u8::MAX)?; // Opaque VP8L alpha lane.
    write_simple_table(&mut writer, 0)?; // Distance codes are unused.
    for &sample in samples {
        write_table_symbol(&mut writer, &table, usize::from(sample))?;
    }
    Ok(writer.into_bytes())
}

struct EncodingTable {
    codes: Vec<(u32, u8)>,
}

fn write_adaptive_table(
    writer: &mut BitWriter,
    frequencies: &[u32],
) -> Result<EncodingTable, AlphaEncodeError> {
    let mut ranked = frequencies
        .iter()
        .copied()
        .enumerate()
        .filter_map(|(symbol, frequency)| (frequency != 0).then_some((frequency, symbol)))
        .collect::<Vec<_>>();
    if ranked.is_empty() {
        ranked.push((1, 0));
    }
    ranked.sort_unstable_by(|left, right| right.0.cmp(&left.0).then_with(|| left.1.cmp(&right.1)));

    let mut lengths = vec![0_u8; frequencies.len()];
    if ranked.len() == 1 {
        lengths[ranked[0].1] = 1;
    } else {
        let floor_log = usize::BITS - 1 - ranked.len().leading_zeros();
        let base = 1_usize << floor_log;
        let short_count = base
            .checked_mul(2)
            .and_then(|count| count.checked_sub(ranked.len()))
            .ok_or(AlphaEncodeError::SizeOverflow)?;
        for (rank, (_, symbol)) in ranked.iter().enumerate() {
            lengths[*symbol] = if rank < short_count {
                floor_log as u8
            } else {
                floor_log as u8 + 1
            };
        }
    }
    write_normal_table(writer, &lengths)?;
    canonical_table(&lengths)
}

fn write_normal_table(writer: &mut BitWriter, lengths: &[u8]) -> Result<(), AlphaEncodeError> {
    write_bits(writer, 0, 1)?;
    write_bits(writer, 15, 4)?;
    for symbol in CODE_LENGTH_CODE_ORDER {
        write_bits(writer, if symbol <= 15 { 4 } else { 0 }, 3)?;
    }
    write_bits(writer, 0, 1)?;
    for &length in lengths {
        write_canonical_symbol(writer, u32::from(length), 4)?;
    }
    Ok(())
}

fn canonical_table(lengths: &[u8]) -> Result<EncodingTable, AlphaEncodeError> {
    let mut symbols = lengths
        .iter()
        .copied()
        .enumerate()
        .filter_map(|(symbol, length)| (length != 0).then_some((length, symbol)))
        .collect::<Vec<_>>();
    symbols.sort_unstable();
    let mut codes = vec![(0_u32, 0_u8); lengths.len()];
    if symbols.len() == 1 {
        codes[symbols[0].1] = (0, 0);
        return Ok(EncodingTable { codes });
    }
    let mut code = 0_u32;
    let mut previous_length = 0_u8;
    for (length, symbol) in symbols {
        code <<= u32::from(length - previous_length);
        codes[symbol] = (code, length);
        code = code.checked_add(1).ok_or(AlphaEncodeError::SizeOverflow)?;
        previous_length = length;
    }
    Ok(EncodingTable { codes })
}

fn write_table_symbol(
    writer: &mut BitWriter,
    table: &EncodingTable,
    symbol: usize,
) -> Result<(), AlphaEncodeError> {
    let (code, width) = table
        .codes
        .get(symbol)
        .copied()
        .ok_or(AlphaEncodeError::SizeOverflow)?;
    write_canonical_symbol(writer, code, width)
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
    if width == 0 {
        return Ok(());
    }
    let wire_code = canonical_code.reverse_bits() >> (u32::BITS - u32::from(width));
    write_bits(writer, wire_code, width)
}

fn write_bits(writer: &mut BitWriter, value: u32, count: u8) -> Result<(), AlphaEncodeError> {
    writer
        .write_bits(value, count)
        .map_err(|_| AlphaEncodeError::AllocationFailed)
}

#[cfg(test)]
#[path = "encode_tests.rs"]
mod tests;
