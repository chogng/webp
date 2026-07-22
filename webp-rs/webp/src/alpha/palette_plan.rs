//! Color-indexing plans for low-cardinality alpha planes.

use crate::BitWriter;
use crate::alpha::AlphaEncodeError;
use crate::alpha::symbol_plan::write_adaptive_table;
use crate::vp8l::huffman::symbol_writer::write_table_symbol;

const MAX_PALETTE_SIZE: usize = 16;
const GREEN_ALPHABET_SIZE: usize = 256 + 24;
const CHANNEL_ALPHABET_SIZE: usize = 256;
const DISTANCE_ALPHABET_SIZE: usize = 40;

pub(super) struct PalettePlan {
    pub(super) entries: Vec<u8>,
    pub(super) indexed_samples: Vec<u8>,
    pub(super) indexed_width: usize,
}

pub(super) fn make_plan(
    samples: &[u8],
    width: usize,
) -> Result<Option<PalettePlan>, AlphaEncodeError> {
    let mut present = [false; 256];
    let mut color_count = 0_usize;
    for &sample in samples {
        if !present[usize::from(sample)] {
            present[usize::from(sample)] = true;
            color_count += 1;
            if color_count > MAX_PALETTE_SIZE {
                return Ok(None);
            }
        }
    }
    let entries = present
        .into_iter()
        .enumerate()
        .filter_map(|(sample, used)| used.then_some(sample as u8))
        .collect::<Vec<_>>();
    let mut indices = [0_u8; 256];
    for (index, &sample) in entries.iter().enumerate() {
        indices[usize::from(sample)] = index as u8;
    }
    let indices_per_byte = match entries.len() {
        1..=2 => 8,
        3..=4 => 4,
        5..=16 => 2,
        _ => return Ok(None),
    };
    let indexed_width = width.div_ceil(indices_per_byte);
    let indexed_len = indexed_width
        .checked_mul(samples.len() / width)
        .ok_or(AlphaEncodeError::SizeOverflow)?;
    let mut indexed_samples = Vec::new();
    indexed_samples
        .try_reserve_exact(indexed_len)
        .map_err(|_| AlphaEncodeError::AllocationFailed)?;
    let bits_per_index = 8 / indices_per_byte;
    for row in samples.chunks_exact(width) {
        for group in row.chunks(indices_per_byte) {
            let mut packed = 0_u8;
            for (position, &sample) in group.iter().enumerate() {
                packed |= indices[usize::from(sample)] << (position * bits_per_index);
            }
            indexed_samples.push(packed);
        }
    }
    Ok(Some(PalettePlan {
        entries,
        indexed_samples,
        indexed_width,
    }))
}

pub(super) fn write_transform(
    writer: &mut BitWriter,
    entries: &[u8],
) -> Result<(), AlphaEncodeError> {
    write_bits(writer, 1, 1)?; // Color-indexing transform follows.
    write_bits(writer, 3, 2)?;
    write_bits(
        writer,
        u32::try_from(entries.len() - 1).map_err(|_| AlphaEncodeError::SizeOverflow)?,
        8,
    )?;
    write_palette_image(writer, entries)?;
    write_bits(writer, 0, 1) // Transform-list terminator.
}

fn write_palette_image(writer: &mut BitWriter, entries: &[u8]) -> Result<(), AlphaEncodeError> {
    let mut green_frequencies = [0_u32; GREEN_ALPHABET_SIZE];
    let mut alpha_frequencies = [0_u32; CHANNEL_ALPHABET_SIZE];
    let mut deltas = Vec::new();
    deltas
        .try_reserve_exact(entries.len())
        .map_err(|_| AlphaEncodeError::AllocationFailed)?;
    let mut previous = 0_u8;
    for (index, &entry) in entries.iter().enumerate() {
        let green = if index == 0 {
            entry
        } else {
            entry.wrapping_sub(previous)
        };
        let alpha = if index == 0 { u8::MAX } else { 0 };
        increment(&mut green_frequencies, usize::from(green))?;
        increment(&mut alpha_frequencies, usize::from(alpha))?;
        deltas.push((green, alpha));
        previous = entry;
    }

    write_bits(writer, 0, 1)?; // No color cache in the palette subimage.
    let green = write_adaptive_table(writer, &green_frequencies)?;
    let red = write_adaptive_table(writer, &[0; CHANNEL_ALPHABET_SIZE])?;
    let blue = write_adaptive_table(writer, &[0; CHANNEL_ALPHABET_SIZE])?;
    let alpha = write_adaptive_table(writer, &alpha_frequencies)?;
    let _distance = write_adaptive_table(writer, &[0; DISTANCE_ALPHABET_SIZE])?;
    for (green_delta, alpha_delta) in deltas {
        write_table_symbol(writer, &green, usize::from(green_delta))?;
        write_table_symbol(writer, &red, 0)?;
        write_table_symbol(writer, &blue, 0)?;
        write_table_symbol(writer, &alpha, usize::from(alpha_delta))?;
    }
    Ok(())
}

fn increment(table: &mut [u32], symbol: usize) -> Result<(), AlphaEncodeError> {
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
#[path = "palette_plan_tests.rs"]
mod tests;
