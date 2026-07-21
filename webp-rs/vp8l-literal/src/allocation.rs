use webp_core::DecodeError;
use webp_core::DecodeErrorKind;
use webp_vp8l_color_cache::MAX_COLOR_CACHE_BITS;
use webp_vp8l_color_cache::MIN_COLOR_CACHE_BITS;

/// Bounds the allocations that coexist while entropy output becomes RGBA.
///
/// The packed ARGB output, optional color-cache entries, and final RGBA bytes
/// all coexist while the decoder allocates the final image. This deliberately
/// treats vector capacities as their maximum configured sizes, avoiding an
/// allocation-limit bypass through a tiny image paired with a large cache.
pub(super) fn check_allocation_budget(
    pixels: usize,
    rgba_len: usize,
    color_cache_size: usize,
    retained_bytes: usize,
    max_alloc_bytes: usize,
) -> Result<(), DecodeError> {
    let packed_bytes = pixels.checked_mul(size_of::<u32>()).ok_or_else(|| {
        DecodeError::new(
            DecodeErrorKind::LimitExceeded,
            None,
            "packed VP8L output byte size overflow",
        )
    })?;
    let cache_bytes = color_cache_size
        .checked_mul(size_of::<u32>())
        .ok_or_else(|| {
            DecodeError::new(
                DecodeErrorKind::LimitExceeded,
                None,
                "VP8L color-cache byte size overflow",
            )
        })?;
    let total = retained_bytes
        .checked_add(packed_bytes)
        .and_then(|value| value.checked_add(cache_bytes))
        .and_then(|value| value.checked_add(rgba_len))
        .ok_or_else(|| {
            DecodeError::new(
                DecodeErrorKind::LimitExceeded,
                None,
                "VP8L decoder allocation budget overflow",
            )
        })?;
    if total > max_alloc_bytes {
        return Err(DecodeError::new(
            DecodeErrorKind::LimitExceeded,
            None,
            "VP8L decoder allocations exceed configured allocation limit",
        ));
    }
    Ok(())
}

pub(super) fn checked_transform_bytes(
    entries: usize,
    entry_size: usize,
    overflow_message: &'static str,
) -> Result<usize, DecodeError> {
    entries
        .checked_mul(entry_size)
        .ok_or_else(|| DecodeError::new(DecodeErrorKind::LimitExceeded, None, overflow_message))
}

/// Verifies the brief conversion overlap between an entropy-decoded packed
/// color subimage and its compact coefficient table.
pub(super) fn check_transient_transform_allocation(
    retained_bytes: usize,
    packed_bytes: usize,
    multiplier_bytes: usize,
    max_alloc_bytes: usize,
) -> Result<(), DecodeError> {
    let total = retained_bytes
        .checked_add(packed_bytes)
        .and_then(|value| value.checked_add(multiplier_bytes))
        .ok_or_else(|| {
            DecodeError::new(
                DecodeErrorKind::LimitExceeded,
                None,
                "VP8L color-transform conversion allocation size overflow",
            )
        })?;
    if total > max_alloc_bytes {
        return Err(DecodeError::new(
            DecodeErrorKind::LimitExceeded,
            None,
            "VP8L color-transform conversion exceeds allocation limit",
        ));
    }
    Ok(())
}

/// Bounds the brief overlap while a decoded packed palette becomes the
/// retained palette representation.
pub(super) fn check_transient_indexing_palette_allocation(
    retained_bytes: usize,
    packed_bytes: usize,
    palette_bytes: usize,
    max_alloc_bytes: usize,
) -> Result<(), DecodeError> {
    let total = retained_bytes
        .checked_add(packed_bytes)
        .and_then(|value| value.checked_add(palette_bytes))
        .ok_or_else(|| {
            DecodeError::new(
                DecodeErrorKind::LimitExceeded,
                None,
                "VP8L color-indexing palette conversion allocation size overflow",
            )
        })?;
    if total > max_alloc_bytes {
        return Err(DecodeError::new(
            DecodeErrorKind::LimitExceeded,
            None,
            "VP8L color-indexing palette conversion exceeds allocation limit",
        ));
    }
    Ok(())
}

pub(super) fn pixel_count(width: u32, height: u32) -> Result<usize, DecodeError> {
    usize::try_from(u64::from(width) * u64::from(height)).map_err(|_| {
        DecodeError::new(
            DecodeErrorKind::LimitExceeded,
            None,
            "image pixel count does not fit platform usize",
        )
    })
}

pub(super) fn color_cache_size(color_cache_bits: Option<u8>) -> Result<usize, DecodeError> {
    match color_cache_bits {
        None => Ok(0),
        Some(cache_bits) => {
            if !(MIN_COLOR_CACHE_BITS..=MAX_COLOR_CACHE_BITS).contains(&cache_bits) {
                return Err(DecodeError::new(
                    DecodeErrorKind::InvalidBitstream,
                    None,
                    "VP8L color-cache bits must be in 1..=11",
                ));
            }
            Ok(1_usize << cache_bits)
        }
    }
}
