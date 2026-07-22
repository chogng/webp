use crate::DecodeError;
use crate::DecodeErrorKind;
use crate::vp8l::allocation::checked_transform_bytes;
use crate::vp8l::allocation::pixel_count;
use crate::vp8l::pixel::pack_argb;
use crate::vp8l::transforms::indexing::Palette;
use crate::vp8l::transforms::indexing::TRANSPARENT_BLACK;

/// Reverses VP8L color indexing in packed ARGB form.  The decoder keeps the
/// narrow entropy output alive until the expanded row-major output is fully
/// initialized, so this explicitly accounts for both buffers plus the final
/// RGBA allocation and all retained transform tables.
pub(in crate::vp8l) fn inverse_color_indexing_argb(
    pixels: &mut Vec<u32>,
    descriptor: crate::vp8l::header::ColorIndexingDescriptor,
    palette: &Palette,
    retained_bytes: usize,
    final_rgba_bytes: usize,
    max_alloc_bytes: usize,
) -> Result<(), DecodeError> {
    let packing = palette.packing();
    let indices_per_pixel = usize::from(packing.indices_per_pixel());
    let expected_bundle = 1_usize << descriptor.width_bits;
    if indices_per_pixel != expected_bundle {
        return Err(DecodeError::new(
            DecodeErrorKind::InvalidBitstream,
            None,
            "VP8L color-indexing descriptor does not match palette packing",
        ));
    }
    if descriptor.image_width_after
        != palette
            .packing()
            .packed_width(descriptor.image_width_before)
            .map_err(|_| {
                DecodeError::new(
                    DecodeErrorKind::InvalidBitstream,
                    None,
                    "VP8L color-indexing packed width is invalid",
                )
            })?
    {
        return Err(DecodeError::new(
            DecodeErrorKind::InvalidBitstream,
            None,
            "VP8L color-indexing packed width does not match descriptor",
        ));
    }

    let packed_pixels = pixel_count(descriptor.image_width_after, descriptor.image_height)?;
    if pixels.len() != packed_pixels {
        return Err(DecodeError::new(
            DecodeErrorKind::InvalidBitstream,
            None,
            "VP8L color-indexing output length does not match packed dimensions",
        ));
    }
    let expanded_pixels = pixel_count(descriptor.image_width_before, descriptor.image_height)?;
    let packed_bytes = checked_transform_bytes(
        packed_pixels,
        size_of::<u32>(),
        "VP8L color-indexing packed image byte size overflow",
    )?;
    let expanded_bytes = checked_transform_bytes(
        expanded_pixels,
        size_of::<u32>(),
        "VP8L color-indexing expanded image byte size overflow",
    )?;
    let total = retained_bytes
        .checked_add(packed_bytes)
        .and_then(|value| value.checked_add(expanded_bytes))
        .and_then(|value| value.checked_add(final_rgba_bytes))
        .ok_or_else(|| {
            DecodeError::new(
                DecodeErrorKind::LimitExceeded,
                None,
                "VP8L color-indexing expansion allocation size overflow",
            )
        })?;
    if total > max_alloc_bytes {
        return Err(DecodeError::new(
            DecodeErrorKind::LimitExceeded,
            None,
            "VP8L color-indexing expansion exceeds allocation limit",
        ));
    }

    let width_before = usize::try_from(descriptor.image_width_before).map_err(|_| {
        DecodeError::new(
            DecodeErrorKind::LimitExceeded,
            None,
            "VP8L color-indexing image width does not fit usize",
        )
    })?;
    let width_after = usize::try_from(descriptor.image_width_after).map_err(|_| {
        DecodeError::new(
            DecodeErrorKind::LimitExceeded,
            None,
            "VP8L color-indexing packed width does not fit usize",
        )
    })?;
    let bits_per_index = u32::from(packing.bits_per_index());
    let mask = (1_u16 << bits_per_index) - 1;

    let mut expanded = Vec::new();
    expanded.try_reserve_exact(expanded_pixels).map_err(|_| {
        DecodeError::new(
            DecodeErrorKind::AllocationFailed,
            None,
            "VP8L color-indexing expanded output allocation failed",
        )
    })?;
    for row in pixels.chunks_exact(width_after) {
        for x in 0..width_before {
            let packed = (row[x / indices_per_pixel] >> 8) as u8;
            let shift = u32::try_from(x % indices_per_pixel)
                .expect("VP8L color-indexing shift fits u32")
                * bits_per_index;
            let index = usize::from((u16::from(packed) >> shift) & mask);
            let color = palette.get(index).unwrap_or(TRANSPARENT_BLACK);
            expanded.push(pack_argb(color.red, color.green, color.blue, color.alpha));
        }
    }
    *pixels = expanded;
    Ok(())
}
