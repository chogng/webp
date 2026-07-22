//! Complete entropy-token output for headerless VP8L alpha payload writing.

use crate::vp8l::huffman::EncodingTable;
use crate::vp8l::huffman::table_wire_symbol;
#[cfg(any(feature = "alpha-benchmark-internals", test))]
use crate::vp8l::huffman::write_table_symbol;
use webp_utils::BitWriter;

use super::AlphaEncodeError;
use super::backward_references as encode_lz77;
use super::backward_references::Token;

pub(super) const MAX_TOKEN_PACKET_BITS: u8 = 58;
const MAX_BITS_PER_SAMPLE: usize = 15;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WriterVariant {
    #[cfg(any(feature = "alpha-benchmark-internals", test))]
    Reference,
    #[cfg(any(feature = "alpha-benchmark-internals", test))]
    PacketReference,
    Packed,
}

#[cfg(feature = "alpha-benchmark-internals")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[doc(hidden)]
pub enum BenchmarkWriterVariant {
    Reference,
    PacketReference,
    Packed,
}

#[cfg(feature = "alpha-benchmark-internals")]
impl From<BenchmarkWriterVariant> for WriterVariant {
    fn from(value: BenchmarkWriterVariant) -> Self {
        match value {
            BenchmarkWriterVariant::Reference => Self::Reference,
            BenchmarkWriterVariant::PacketReference => Self::PacketReference,
            BenchmarkWriterVariant::Packed => Self::Packed,
        }
    }
}

#[cfg(feature = "alpha-benchmark-internals")]
thread_local! {
    static BENCHMARK_WRITER_VARIANT: std::cell::Cell<BenchmarkWriterVariant> =
        const { std::cell::Cell::new(BenchmarkWriterVariant::Packed) };
}

#[cfg(feature = "alpha-benchmark-internals")]
#[doc(hidden)]
pub fn set_benchmark_writer_variant(variant: BenchmarkWriterVariant) {
    BENCHMARK_WRITER_VARIANT.set(variant);
}

fn selected_variant() -> WriterVariant {
    #[cfg(feature = "alpha-benchmark-internals")]
    {
        BENCHMARK_WRITER_VARIANT.get().into()
    }
    #[cfg(not(feature = "alpha-benchmark-internals"))]
    {
        WriterVariant::Packed
    }
}

pub(super) fn write_tokens(
    samples: &[u8],
    image_width: usize,
    match_table: &mut encode_lz77::MatchTable,
    cached_tokens: Option<&[u32]>,
    prefix: BitWriter,
    green: &EncodingTable,
    distance: &EncodingTable,
) -> Result<Vec<u8>, AlphaEncodeError> {
    write_tokens_with_variant(
        samples,
        image_width,
        match_table,
        cached_tokens,
        prefix,
        green,
        distance,
        selected_variant(),
    )
}

#[allow(clippy::too_many_arguments)]
fn write_tokens_with_variant(
    samples: &[u8],
    image_width: usize,
    match_table: &mut encode_lz77::MatchTable,
    cached_tokens: Option<&[u32]>,
    prefix: BitWriter,
    green: &EncodingTable,
    distance: &EncodingTable,
    variant: WriterVariant,
) -> Result<Vec<u8>, AlphaEncodeError> {
    match variant {
        #[cfg(any(feature = "alpha-benchmark-internals", test))]
        WriterVariant::Reference => {
            let mut writer = prefix;
            visit_tokens(samples, match_table, cached_tokens, |token| {
                write_reference_token(&mut writer, green, distance, image_width, token)
            })?;
            Ok(writer.into_bytes())
        }
        #[cfg(any(feature = "alpha-benchmark-internals", test))]
        WriterVariant::PacketReference => {
            let mut writer = prefix;
            visit_tokens(samples, match_table, cached_tokens, |token| {
                let packet = packet_for_token(green, distance, image_width, token)?;
                append_packet_reference(&mut writer, packet)
            })?;
            Ok(writer.into_bytes())
        }
        WriterVariant::Packed => {
            let mut writer = PackedTokenSink::from_prefix(prefix, samples.len())?;
            visit_tokens(samples, match_table, cached_tokens, |token| {
                writer.append(packet_for_token(green, distance, image_width, token)?)
            })?;
            writer.finish()
        }
    }
}

fn visit_tokens<E>(
    samples: &[u8],
    match_table: &mut encode_lz77::MatchTable,
    cached_tokens: Option<&[u32]>,
    mut visit: impl FnMut(Token) -> Result<(), E>,
) -> Result<(), E> {
    if let Some(tokens) = cached_tokens {
        for &token in tokens {
            visit(encode_lz77::unpack(token))?;
        }
        return Ok(());
    }
    match_table.reset();
    encode_lz77::walk(samples, match_table, visit)
}

#[cfg(any(feature = "alpha-benchmark-internals", test))]
fn write_reference_token(
    writer: &mut BitWriter,
    green: &EncodingTable,
    distance: &EncodingTable,
    image_width: usize,
    token: Token,
) -> Result<(), AlphaEncodeError> {
    match token {
        Token::Literal(sample) => Ok(write_table_symbol(writer, green, usize::from(sample))?),
        Token::Copy {
            length,
            distance: copy_distance,
        } => {
            let length = encode_lz77::prefix_code(length, encode_lz77::LENGTH_PREFIX_COUNT)
                .ok_or(AlphaEncodeError::SizeOverflow)?;
            write_table_symbol(
                writer,
                green,
                encode_lz77::CHANNEL_ALPHABET_SIZE + length.symbol,
            )?;
            write_bits(writer, length.extra, length.extra_bits)?;
            let distance_code = encode_lz77::distance_code(image_width, copy_distance);
            let distance_prefix =
                encode_lz77::prefix_code(distance_code, encode_lz77::DISTANCE_ALPHABET_SIZE)
                    .ok_or(AlphaEncodeError::SizeOverflow)?;
            write_table_symbol(writer, distance, distance_prefix.symbol)?;
            write_bits(writer, distance_prefix.extra, distance_prefix.extra_bits)
        }
    }
}

#[cfg(any(feature = "alpha-benchmark-internals", test))]
fn write_bits(writer: &mut BitWriter, value: u32, count: u8) -> Result<(), AlphaEncodeError> {
    writer
        .write_bits(value, count)
        .map_err(|_| AlphaEncodeError::AllocationFailed)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TokenPacket {
    bits: u64,
    width: u8,
}

impl TokenPacket {
    const fn new() -> Self {
        Self { bits: 0, width: 0 }
    }

    fn push(&mut self, bits: u32, width: u8) -> Result<(), AlphaEncodeError> {
        if width > u32::BITS as u8 {
            return Err(AlphaEncodeError::SizeOverflow);
        }
        let next = self
            .width
            .checked_add(width)
            .ok_or(AlphaEncodeError::SizeOverflow)?;
        if next > u64::BITS as u8 {
            return Err(AlphaEncodeError::SizeOverflow);
        }
        let mask = match width {
            0 => 0,
            32 => u64::from(u32::MAX),
            _ => (1_u64 << width) - 1,
        };
        self.bits |= (u64::from(bits) & mask) << self.width;
        self.width = next;
        Ok(())
    }

    fn push_symbol(
        &mut self,
        table: &EncodingTable,
        symbol: usize,
    ) -> Result<(), AlphaEncodeError> {
        let (bits, width) = table_wire_symbol(table, symbol)?;
        self.push(bits, width)
    }
}

fn packet_for_token(
    green: &EncodingTable,
    distance: &EncodingTable,
    image_width: usize,
    token: Token,
) -> Result<TokenPacket, AlphaEncodeError> {
    let mut packet = TokenPacket::new();
    match token {
        Token::Literal(sample) => packet.push_symbol(green, usize::from(sample))?,
        Token::Copy {
            length,
            distance: copy_distance,
        } => {
            let length = encode_lz77::prefix_code(length, encode_lz77::LENGTH_PREFIX_COUNT)
                .ok_or(AlphaEncodeError::SizeOverflow)?;
            packet.push_symbol(green, encode_lz77::CHANNEL_ALPHABET_SIZE + length.symbol)?;
            packet.push(length.extra, length.extra_bits)?;
            let distance_code = encode_lz77::distance_code(image_width, copy_distance);
            let distance_prefix =
                encode_lz77::prefix_code(distance_code, encode_lz77::DISTANCE_ALPHABET_SIZE)
                    .ok_or(AlphaEncodeError::SizeOverflow)?;
            packet.push_symbol(distance, distance_prefix.symbol)?;
            packet.push(distance_prefix.extra, distance_prefix.extra_bits)?;
        }
    }
    if packet.width > MAX_TOKEN_PACKET_BITS {
        return Err(AlphaEncodeError::SizeOverflow);
    }
    Ok(packet)
}

#[cfg(any(feature = "alpha-benchmark-internals", test))]
fn append_packet_reference(
    writer: &mut BitWriter,
    packet: TokenPacket,
) -> Result<(), AlphaEncodeError> {
    let lower = packet.width.min(u32::BITS as u8);
    write_bits(writer, packet.bits as u32, lower)?;
    if packet.width > u32::BITS as u8 {
        write_bits(
            writer,
            (packet.bits >> u32::BITS) as u32,
            packet.width - u32::BITS as u8,
        )?;
    }
    Ok(())
}

struct PackedTokenSink {
    data: Vec<u8>,
    accumulator: u64,
    used: u8,
}

impl PackedTokenSink {
    fn from_prefix(prefix: BitWriter, sample_count: usize) -> Result<Self, AlphaEncodeError> {
        let bit_len = prefix.bit_len();
        Self::from_parts(prefix.into_bytes(), bit_len, sample_count)
    }

    fn from_parts(
        mut data: Vec<u8>,
        bit_len: usize,
        sample_count: usize,
    ) -> Result<Self, AlphaEncodeError> {
        let full_bytes = bit_len / 8;
        let used = (bit_len % 8) as u8;
        let expected = full_bytes
            .checked_add(usize::from(used != 0))
            .ok_or(AlphaEncodeError::SizeOverflow)?;
        if data.len() != expected {
            return Err(AlphaEncodeError::SizeOverflow);
        }
        let accumulator = if used == 0 {
            0
        } else {
            u64::from(data[full_bytes] & ((1_u8 << used) - 1))
        };
        data.truncate(full_bytes);
        let reserve_bits = sample_count
            .checked_mul(MAX_BITS_PER_SAMPLE)
            .and_then(|bits| bits.checked_add(usize::from(used)))
            .ok_or(AlphaEncodeError::SizeOverflow)?;
        let reserve_bytes = reserve_bits
            .checked_add(7)
            .ok_or(AlphaEncodeError::SizeOverflow)?
            / 8;
        data.try_reserve_exact(reserve_bytes)
            .map_err(|_| AlphaEncodeError::AllocationFailed)?;
        Ok(Self {
            data,
            accumulator,
            used,
        })
    }

    fn append(&mut self, packet: TokenPacket) -> Result<(), AlphaEncodeError> {
        if packet.width > MAX_TOKEN_PACKET_BITS || self.used >= u32::BITS as u8 {
            return Err(AlphaEncodeError::SizeOverflow);
        }
        let mask = if packet.width == 0 {
            0
        } else {
            (1_u64 << packet.width) - 1
        };
        let mut pending =
            u128::from(self.accumulator) | (u128::from(packet.bits & mask) << self.used);
        let mut used = self
            .used
            .checked_add(packet.width)
            .ok_or(AlphaEncodeError::SizeOverflow)?;
        while used >= u32::BITS as u8 {
            self.extend(&(pending as u32).to_le_bytes())?;
            pending >>= u32::BITS;
            used -= u32::BITS as u8;
        }
        self.accumulator = pending as u64;
        self.used = used;
        Ok(())
    }

    fn extend(&mut self, bytes: &[u8]) -> Result<(), AlphaEncodeError> {
        let end = self
            .data
            .len()
            .checked_add(bytes.len())
            .ok_or(AlphaEncodeError::SizeOverflow)?;
        if end > self.data.capacity() {
            return Err(AlphaEncodeError::AllocationFailed);
        }
        self.data.extend_from_slice(bytes);
        Ok(())
    }

    fn finish(mut self) -> Result<Vec<u8>, AlphaEncodeError> {
        let remaining = usize::from(self.used).div_ceil(8);
        let tail = self.accumulator.to_le_bytes();
        self.extend(&tail[..remaining])?;
        Ok(self.data)
    }
}

#[cfg(test)]
#[path = "encode_token_output_tests.rs"]
mod tests;
