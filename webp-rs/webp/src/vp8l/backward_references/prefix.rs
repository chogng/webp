//! Shared VP8L length and distance prefix mapping.

use crate::vp8l::huffman::symbol_writer::WireWriteError;

pub(crate) fn encode_prefix(
    value: usize,
    prefix_count: usize,
) -> Result<(usize, (u32, u8)), WireWriteError> {
    for prefix in 0..prefix_count {
        if prefix < 4 {
            if value == prefix + 1 {
                return Ok((prefix, (0, 0)));
            }
            continue;
        }
        let prefix = u8::try_from(prefix).map_err(|_| WireWriteError::SizeOverflow)?;
        let extra_bits = (prefix - 2) >> 1;
        let offset = (2_usize + usize::from(prefix & 1)) << extra_bits;
        let base = offset.checked_add(1).ok_or(WireWriteError::SizeOverflow)?;
        let range = 1_usize << usize::from(extra_bits);
        if value >= base && value < base.saturating_add(range) {
            return Ok((
                usize::from(prefix),
                (
                    u32::try_from(value - base).map_err(|_| WireWriteError::SizeOverflow)?,
                    extra_bits,
                ),
            ));
        }
    }
    Err(WireWriteError::SizeOverflow)
}
