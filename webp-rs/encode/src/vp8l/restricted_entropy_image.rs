//! Single-group entropy coding for VP8L transform subimages.
//!
//! libwebp calls this path `EncodeImageNoHuffman`, despite it still building
//! and writing five Huffman tables.  The name refers to the absence of a
//! colour cache and a meta-Huffman image, not the absence of Huffman coding.

use super::entropy_plan::EntropyPlan;
use super::packet_sink::PackedTokenWriter;
use super::predictor_plan::PredictorPlan;
use super::token_stream::{ParseMode, ResidualImage, TokenStream};
use super::{BitWriter, EncodeError, write_bits};

/// Writes a transform subimage with one complete Huffman-table group.
///
/// This is deliberately separate from the main-image writers. Transform
/// images omit the level-zero meta-Huffman flag, must not enable a colour
/// cache, and use the standard LZ77/RLE backward-reference parse. The
/// canonical-table builder represents a one-symbol tree with a zero-bit code
/// after its table has been written, matching libwebp's cleanup rule.
pub(super) fn write_restricted_entropy_image(
    writer: &mut BitWriter,
    rgba: &[u8],
    width: usize,
) -> Result<(), EncodeError> {
    let stream = collect_restricted_stream(rgba, width)?;
    let plan = EntropyPlan::build_compact_for_stream(stream.statistics())?;

    write_bits(writer, 0, 1)?; // No colour cache; no level-zero meta flag.
    plan.write_tables(writer)?;
    let prefix = std::mem::take(writer);
    let mut packed = PackedTokenWriter::from_prefix(prefix, plan.token_bits())?;
    for &token in stream.tokens() {
        packed.write_token(token, plan.tables())?;
    }
    *writer = packed.into_prefix()?;
    Ok(())
}

fn collect_restricted_stream(rgba: &[u8], width: usize) -> Result<TokenStream, EncodeError> {
    let residuals =
        ResidualImage::collect_with_predictor(rgba, width, false, None, &PredictorPlan::None)?;
    let parse = residuals.parse_compressed(ParseMode::Greedy)?;
    // `EncodeImageNoHuffman` fixes cache_bits to zero, but still enables both
    // standard matching and RLE in its backward-reference search.
    TokenStream::collect_compressed_from_parse(&residuals, &parse, 0)
}

#[cfg(test)]
#[path = "restricted_entropy_image_tests.rs"]
mod tests;
