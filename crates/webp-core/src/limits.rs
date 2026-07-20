use crate::{DecodeError, DecodeErrorKind};

/// Parsing policy for inputs where the WebP specifications and legacy readers
/// intentionally differ.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum CompatibilityProfile {
    /// Reject malformed containers according to the format specification.
    #[default]
    SpecStrict,
    /// Accept selected legacy forms supported by libwebp.
    LibwebpCompatible,
}

/// Product-level resource bounds applied before allocation or expensive work.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodeLimits {
    pub max_input_bytes: usize,
    pub max_width: u32,
    pub max_height: u32,
    pub max_pixels: u64,
    pub max_frames: u32,
    pub max_total_frame_pixels: u64,
    pub max_metadata_bytes: usize,
    pub max_alloc_bytes: usize,
    pub max_work_units: u64,
}

impl Default for DecodeLimits {
    fn default() -> Self {
        Self {
            max_input_bytes: 512 * 1024 * 1024,
            max_width: 16_777_216,
            max_height: 16_777_216,
            max_pixels: 268_435_456,
            max_frames: 16_384,
            max_total_frame_pixels: 1_073_741_824,
            max_metadata_bytes: 64 * 1024 * 1024,
            max_alloc_bytes: 1_073_741_824,
            max_work_units: 1_073_741_824,
        }
    }
}

impl DecodeLimits {
    pub fn check_input_len(&self, input_len: usize) -> Result<(), DecodeError> {
        if input_len > self.max_input_bytes {
            return Err(DecodeError::new(
                DecodeErrorKind::LimitExceeded,
                None,
                "input exceeds configured byte limit",
            ));
        }
        Ok(())
    }

    pub fn check_image(&self, width: u32, height: u32) -> Result<(), DecodeError> {
        if width > self.max_width || height > self.max_height {
            return Err(DecodeError::new(
                DecodeErrorKind::LimitExceeded,
                None,
                "image dimensions exceed configured limit",
            ));
        }
        let pixels = u64::from(width)
            .checked_mul(u64::from(height))
            .ok_or_else(|| {
                DecodeError::new(DecodeErrorKind::LimitExceeded, None, "pixel count overflow")
            })?;
        if pixels > self.max_pixels {
            return Err(DecodeError::new(
                DecodeErrorKind::LimitExceeded,
                None,
                "image pixel count exceeds configured limit",
            ));
        }
        Ok(())
    }

    #[must_use]
    pub const fn work_budget(&self) -> WorkBudget {
        WorkBudget::new(self.max_work_units)
    }
}

/// Deterministic counter used to bound CPU work independently of wall-clock time.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkBudget {
    remaining: u64,
}

impl WorkBudget {
    #[must_use]
    pub const fn new(units: u64) -> Self {
        Self { remaining: units }
    }

    #[must_use]
    pub const fn remaining(&self) -> u64 {
        self.remaining
    }

    #[inline]
    pub fn consume(&mut self, units: u64) -> Result<(), DecodeError> {
        self.remaining = self.remaining.checked_sub(units).ok_or_else(|| {
            DecodeError::new(
                DecodeErrorKind::LimitExceeded,
                None,
                "decode work budget exhausted",
            )
        })?;
        Ok(())
    }
}

/// Returns `width * height * channels` without target-word-size truncation.
pub fn checked_image_bytes(width: u32, height: u32, channels: usize) -> Result<usize, DecodeError> {
    let pixels = usize::try_from(u64::from(width) * u64::from(height)).map_err(|_| {
        DecodeError::new(
            DecodeErrorKind::InvalidParameter,
            None,
            "image size does not fit usize",
        )
    })?;
    pixels.checked_mul(channels).ok_or_else(|| {
        DecodeError::new(
            DecodeErrorKind::InvalidParameter,
            None,
            "image byte size overflow",
        )
    })
}

/// Ensures a rectangle end is representable and contained in `limit`.
pub fn checked_rect_end(origin: u32, extent: u32, limit: u32) -> Result<u32, DecodeError> {
    let end = origin.checked_add(extent).ok_or_else(|| {
        DecodeError::new(
            DecodeErrorKind::InvalidParameter,
            None,
            "rectangle end overflow",
        )
    })?;
    if end > limit {
        return Err(DecodeError::new(
            DecodeErrorKind::InvalidParameter,
            None,
            "rectangle exceeds containing dimension",
        ));
    }
    Ok(end)
}

/// Validates a RIFF chunk at `offset` and returns the offset after its padding.
///
/// `offset` points to the four-byte chunk FourCC; the payload is preceded by a
/// four-byte little-endian size and odd payloads carry one padding byte.
pub fn checked_chunk_end(
    offset: usize,
    payload: u32,
    input_len: usize,
) -> Result<usize, DecodeError> {
    let payload = usize::try_from(payload).map_err(|_| {
        DecodeError::at(
            DecodeErrorKind::InvalidContainer,
            offset,
            "chunk size does not fit usize",
        )
    })?;
    let end = offset
        .checked_add(8)
        .and_then(|value| value.checked_add(payload))
        .and_then(|value| value.checked_add(payload & 1))
        .ok_or_else(|| {
            DecodeError::at(
                DecodeErrorKind::InvalidContainer,
                offset,
                "chunk end overflow",
            )
        })?;
    if end > input_len {
        return Err(DecodeError::at(
            DecodeErrorKind::UnexpectedEof,
            offset,
            "truncated RIFF chunk",
        ));
    }
    Ok(end)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arithmetic_boundaries_are_checked() {
        assert_eq!(checked_image_bytes(0, u32::MAX, 4), Ok(0));
        assert_eq!(checked_image_bytes(1, 1, 4), Ok(4));
        assert_eq!(checked_image_bytes(2, 3, 4), Ok(24));
        assert_eq!(checked_rect_end(3, 4, 7), Ok(7));
        assert!(checked_rect_end(u32::MAX, 1, u32::MAX).is_err());
        assert!(checked_rect_end(7, 1, 7).is_err());
    }

    #[test]
    fn chunk_end_accounts_for_header_and_padding() {
        assert_eq!(checked_chunk_end(12, 0, 20), Ok(20));
        assert_eq!(checked_chunk_end(12, 3, 24), Ok(24));
        assert_eq!(checked_chunk_end(12, 4, 24), Ok(24));
        assert_eq!(
            checked_chunk_end(12, 5, 24).unwrap_err().kind(),
            DecodeErrorKind::UnexpectedEof
        );
    }

    #[test]
    fn work_budget_is_transactional() {
        let mut budget = WorkBudget::new(3);
        assert_eq!(budget.remaining(), 3);
        budget.consume(3).unwrap();
        assert_eq!(budget.remaining(), 0);
        assert_eq!(
            budget.consume(1).unwrap_err().kind(),
            DecodeErrorKind::LimitExceeded
        );
        assert_eq!(budget.remaining(), 0);
    }

    #[test]
    fn configured_limits_apply_before_use() {
        let limits = DecodeLimits {
            max_input_bytes: 5,
            max_width: 4,
            max_height: 3,
            max_pixels: 3,
            ..DecodeLimits::default()
        };
        assert_eq!(limits.check_input_len(4), Ok(()));
        assert_eq!(limits.check_input_len(5), Ok(()));
        assert!(limits.check_input_len(6).is_err());
        assert_eq!(limits.check_image(1, 3), Ok(()));
        assert!(limits.check_image(2, 2).is_err());
        assert!(limits.check_image(3, 1).is_ok());
        assert!(limits.check_image(5, 1).is_err());
        assert!(limits.check_image(1, 4).is_err());

        let zero = DecodeLimits {
            max_input_bytes: 0,
            max_width: 0,
            max_height: 0,
            max_pixels: 0,
            ..DecodeLimits::default()
        };
        assert_eq!(zero.check_input_len(0), Ok(()));
        assert!(zero.check_input_len(1).is_err());
        assert_eq!(zero.check_image(0, 0), Ok(()));
        assert!(zero.check_image(1, 0).is_err());
        assert!(zero.check_image(0, 1).is_err());
    }
}
