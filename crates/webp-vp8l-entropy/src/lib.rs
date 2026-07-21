#![forbid(unsafe_code)]
//! VP8L entropy-stream primitives.
//!
//! This crate deliberately contains no Huffman-table builder yet.  It provides
//! the spec-defined length/distance prefix expansion and the bounds-checked
//! LZ77 copy operation that the entropy decoder will call after decoding its
//! Huffman symbols.

use webp_core::{BitReader, DecodeError, DecodeErrorKind, ShiftedBitReader, WorkBudget};

pub const LENGTH_PREFIX_COUNT: u8 = 24;
pub const DISTANCE_PREFIX_COUNT: u8 = 40;
pub const MAX_BACKWARD_REFERENCE_LENGTH: usize = 4096;

/// Expands a VP8L LZ77 length prefix and consumes its LSB-first extra bits.
pub fn decode_length(
    reader: &mut BitReader<'_>,
    budget: &mut WorkBudget,
    prefix: u8,
) -> Result<usize, DecodeError> {
    let value = decode_prefix_value(reader, budget, prefix, LENGTH_PREFIX_COUNT)?;
    usize::try_from(value).map_err(|_| {
        DecodeError::new(
            DecodeErrorKind::InvalidBitstream,
            None,
            "length does not fit platform usize",
        )
    })
}

/// Expands a VP8L LZ77 distance prefix and consumes its LSB-first extra bits.
pub fn decode_distance(
    reader: &mut BitReader<'_>,
    budget: &mut WorkBudget,
    prefix: u8,
) -> Result<usize, DecodeError> {
    let value = decode_prefix_value(reader, budget, prefix, DISTANCE_PREFIX_COUNT)?;
    usize::try_from(value).map_err(|_| {
        DecodeError::new(
            DecodeErrorKind::InvalidBitstream,
            None,
            "distance code does not fit platform usize",
        )
    })
}

/// Shift-register counterpart of [`decode_length`] for dense VP8L entropy
/// loops. Prefix validation and work accounting are unchanged.
#[inline]
pub fn decode_length_shifted(
    reader: &mut ShiftedBitReader<'_, '_>,
    budget: &mut WorkBudget,
    prefix: u8,
) -> Result<usize, DecodeError> {
    let value = decode_prefix_value_shifted(reader, budget, prefix, LENGTH_PREFIX_COUNT)?;
    usize::try_from(value).map_err(|_| {
        DecodeError::new(
            DecodeErrorKind::InvalidBitstream,
            None,
            "length does not fit platform usize",
        )
    })
}

/// Shift-register counterpart of [`decode_distance`] for dense VP8L entropy
/// loops.
#[inline]
pub fn decode_distance_shifted(
    reader: &mut ShiftedBitReader<'_, '_>,
    budget: &mut WorkBudget,
    prefix: u8,
) -> Result<usize, DecodeError> {
    let value = decode_prefix_value_shifted(reader, budget, prefix, DISTANCE_PREFIX_COUNT)?;
    usize::try_from(value).map_err(|_| {
        DecodeError::new(
            DecodeErrorKind::InvalidBitstream,
            None,
            "distance code does not fit platform usize",
        )
    })
}

/// Converts a decoded VP8L distance code to scan-line distance for `width`.
pub fn distance_code_to_distance(distance_code: usize, width: u32) -> Result<usize, DecodeError> {
    if distance_code == 0 {
        return Err(DecodeError::new(
            DecodeErrorKind::InvalidBitstream,
            None,
            "distance code must be nonzero",
        ));
    }
    if distance_code > 120 {
        return Ok(distance_code - 120);
    }

    let (x, y) = DISTANCE_MAP[distance_code - 1];
    let width = i64::from(width);
    let distance = i64::from(x)
        .checked_add(i64::from(y).checked_mul(width).ok_or_else(|| {
            DecodeError::new(
                DecodeErrorKind::InvalidBitstream,
                None,
                "mapped distance overflow",
            )
        })?)
        .ok_or_else(|| {
            DecodeError::new(
                DecodeErrorKind::InvalidBitstream,
                None,
                "mapped distance overflow",
            )
        })?;
    // The format explicitly clamps plane distances that fall before the first
    // sample of a short row; later validation rejects references not produced.
    usize::try_from(distance.max(1)).map_err(|_| {
        DecodeError::new(
            DecodeErrorKind::InvalidBitstream,
            None,
            "mapped distance does not fit platform usize",
        )
    })
}

/// Appends an overlap-safe VP8L backward reference to an ARGB output stream.
///
/// Each produced pixel consumes one work unit.  Invalid references and an
/// exhausted work budget leave `output` unchanged.
pub fn copy_lz77_pixels(
    output: &mut Vec<u32>,
    length: usize,
    distance: usize,
    output_limit: usize,
    budget: &mut WorkBudget,
) -> Result<(), DecodeError> {
    copy_lz77_pixels_inner::<false>(output, length, distance, output_limit, budget)
}

/// Appends an overlap-safe backward reference when the caller has already
/// reserved every pixel through `output_limit`.
///
/// The VP8L literal decoder validates that invariant before entering its
/// entropy loop.  Keeping this specialized entry point separate leaves the
/// generic [`copy_lz77_pixels`] API responsible for incremental allocation
/// failure reporting, while avoiding a redundant capacity branch for each
/// well-formed copy command.
#[inline]
pub fn copy_lz77_pixels_preallocated(
    output: &mut Vec<u32>,
    length: usize,
    distance: usize,
    output_limit: usize,
    budget: &mut WorkBudget,
) -> Result<(), DecodeError> {
    copy_lz77_pixels_inner::<true>(output, length, distance, output_limit, budget)
}

#[inline]
fn copy_lz77_pixels_inner<const PREALLOCATED: bool>(
    output: &mut Vec<u32>,
    length: usize,
    distance: usize,
    output_limit: usize,
    budget: &mut WorkBudget,
) -> Result<(), DecodeError> {
    validate_copy(output.len(), length, distance, output_limit)?;
    budget.consume(u64::try_from(length).map_err(|_| {
        DecodeError::new(
            DecodeErrorKind::LimitExceeded,
            None,
            "copy length does not fit work counter",
        )
    })?)?;
    if PREALLOCATED {
        debug_assert!(output.capacity().saturating_sub(output.len()) >= length);
    } else {
        let available_capacity = output.capacity().saturating_sub(output.len());
        if available_capacity < length {
            output
                .try_reserve(length - available_capacity)
                .map_err(|_| {
                    DecodeError::new(
                        DecodeErrorKind::AllocationFailed,
                        None,
                        "LZ77 output allocation failed",
                    )
                })?;
        }
    }

    // Copy up to one distance at a time.  A later iteration reads the chunk
    // just appended, which realizes LZ77 overlap without aliasing references.
    let mut remaining = length;
    while remaining != 0 {
        let chunk_len = remaining.min(distance);
        let start = output.len() - distance;
        output.extend_from_within(start..start + chunk_len);
        remaining -= chunk_len;
    }
    Ok(())
}

fn decode_prefix_value(
    reader: &mut BitReader<'_>,
    budget: &mut WorkBudget,
    prefix: u8,
    prefix_count: u8,
) -> Result<u32, DecodeError> {
    if prefix >= prefix_count {
        return Err(DecodeError::new(
            DecodeErrorKind::InvalidBitstream,
            None,
            "invalid VP8L length/distance prefix",
        ));
    }
    budget.consume(1)?;
    if prefix < 4 {
        return Ok(u32::from(prefix) + 1);
    }
    let extra_bits = (prefix - 2) >> 1;
    let offset = (2_u32 + u32::from(prefix & 1)) << extra_bits;
    let extra = reader.read_bits(extra_bits)?;
    Ok(offset + extra + 1)
}

#[inline]
fn decode_prefix_value_shifted(
    reader: &mut ShiftedBitReader<'_, '_>,
    budget: &mut WorkBudget,
    prefix: u8,
    prefix_count: u8,
) -> Result<u32, DecodeError> {
    if prefix >= prefix_count {
        return Err(DecodeError::new(
            DecodeErrorKind::InvalidBitstream,
            None,
            "invalid VP8L length/distance prefix",
        ));
    }
    budget.consume(1)?;
    if prefix < 4 {
        return Ok(u32::from(prefix) + 1);
    }
    let extra_bits = (prefix - 2) >> 1;
    let offset = (2_u32 + u32::from(prefix & 1)) << extra_bits;
    let extra = reader.read_bits(extra_bits)?;
    Ok(offset + extra + 1)
}

fn validate_copy(
    produced: usize,
    length: usize,
    distance: usize,
    output_limit: usize,
) -> Result<(), DecodeError> {
    if length == 0 || length > MAX_BACKWARD_REFERENCE_LENGTH {
        return Err(DecodeError::new(
            DecodeErrorKind::InvalidBitstream,
            None,
            "invalid VP8L backward-reference length",
        ));
    }
    if distance == 0 || distance > produced {
        return Err(DecodeError::new(
            DecodeErrorKind::InvalidBitstream,
            None,
            "backward reference points outside produced pixels",
        ));
    }
    let end = produced.checked_add(length).ok_or_else(|| {
        DecodeError::new(
            DecodeErrorKind::LimitExceeded,
            None,
            "LZ77 output length overflow",
        )
    })?;
    if end > output_limit {
        return Err(DecodeError::new(
            DecodeErrorKind::InvalidBitstream,
            None,
            "backward reference exceeds image output",
        ));
    }
    Ok(())
}

// `(x, y)` offsets from RFC 9649 / the VP8L bitstream specification.
const DISTANCE_MAP: [(i8, i8); 120] = [
    (0, 1),
    (1, 0),
    (1, 1),
    (-1, 1),
    (0, 2),
    (2, 0),
    (1, 2),
    (-1, 2),
    (2, 1),
    (-2, 1),
    (2, 2),
    (-2, 2),
    (0, 3),
    (3, 0),
    (1, 3),
    (-1, 3),
    (3, 1),
    (-3, 1),
    (2, 3),
    (-2, 3),
    (3, 2),
    (-3, 2),
    (0, 4),
    (4, 0),
    (1, 4),
    (-1, 4),
    (4, 1),
    (-4, 1),
    (3, 3),
    (-3, 3),
    (2, 4),
    (-2, 4),
    (4, 2),
    (-4, 2),
    (0, 5),
    (3, 4),
    (-3, 4),
    (4, 3),
    (-4, 3),
    (5, 0),
    (1, 5),
    (-1, 5),
    (5, 1),
    (-5, 1),
    (2, 5),
    (-2, 5),
    (5, 2),
    (-5, 2),
    (4, 4),
    (-4, 4),
    (3, 5),
    (-3, 5),
    (5, 3),
    (-5, 3),
    (0, 6),
    (6, 0),
    (1, 6),
    (-1, 6),
    (6, 1),
    (-6, 1),
    (2, 6),
    (-2, 6),
    (6, 2),
    (-6, 2),
    (4, 5),
    (-4, 5),
    (5, 4),
    (-5, 4),
    (3, 6),
    (-3, 6),
    (6, 3),
    (-6, 3),
    (0, 7),
    (7, 0),
    (1, 7),
    (-1, 7),
    (5, 5),
    (-5, 5),
    (7, 1),
    (-7, 1),
    (4, 6),
    (-4, 6),
    (6, 4),
    (-6, 4),
    (2, 7),
    (-2, 7),
    (7, 2),
    (-7, 2),
    (3, 7),
    (-3, 7),
    (7, 3),
    (-7, 3),
    (5, 6),
    (-5, 6),
    (6, 5),
    (-6, 5),
    (8, 0),
    (4, 7),
    (-4, 7),
    (7, 4),
    (-7, 4),
    (8, 1),
    (8, 2),
    (6, 6),
    (-6, 6),
    (8, 3),
    (5, 7),
    (-5, 7),
    (7, 5),
    (-7, 5),
    (8, 4),
    (6, 7),
    (-6, 7),
    (7, 6),
    (-7, 6),
    (8, 5),
    (7, 7),
    (-7, 7),
    (8, 6),
    (8, 7),
];

#[cfg(test)]
mod tests {
    use super::*;
    use webp_core::BitWriter;

    fn decode_with_extra(prefix: u8, extra: u32, limit: u8) -> u32 {
        let extra_bits = if prefix < 4 { 0 } else { (prefix - 2) >> 1 };
        let mut writer = BitWriter::new();
        writer.write_bits(extra, extra_bits).unwrap();
        let bytes = writer.into_bytes();
        let mut reader = BitReader::new(&bytes);
        let mut budget = WorkBudget::new(1);
        decode_prefix_value(&mut reader, &mut budget, prefix, limit).unwrap()
    }

    #[test]
    fn every_length_prefix_has_its_specified_range() {
        for prefix in 0..LENGTH_PREFIX_COUNT {
            let extra_bits = if prefix < 4 { 0 } else { (prefix - 2) >> 1 };
            let min = decode_with_extra(prefix, 0, LENGTH_PREFIX_COUNT);
            let max = decode_with_extra(
                prefix,
                (1_u32 << extra_bits).saturating_sub(1),
                LENGTH_PREFIX_COUNT,
            );
            assert_eq!(max - min + 1, 1_u32 << extra_bits);
        }
        assert_eq!(decode_with_extra(0, 0, LENGTH_PREFIX_COUNT), 1);
        assert_eq!(decode_with_extra(23, 0, LENGTH_PREFIX_COUNT), 3073);
        assert_eq!(decode_with_extra(23, 1023, LENGTH_PREFIX_COUNT), 4096);
    }

    #[test]
    fn every_distance_prefix_is_accepted_and_prefix_bounds_are_checked() {
        for prefix in 0..DISTANCE_PREFIX_COUNT {
            let extra_bits = if prefix < 4 { 0 } else { (prefix - 2) >> 1 };
            for extra in [0, (1_u32 << extra_bits).saturating_sub(1)] {
                let mut writer = BitWriter::new();
                writer.write_bits(extra, extra_bits).unwrap();
                let bytes = writer.into_bytes();
                let mut reader = BitReader::new(&bytes);
                let mut budget = WorkBudget::new(1);
                let expected = if prefix < 4 {
                    usize::from(prefix) + 1
                } else {
                    let offset = (2_usize + usize::from(prefix & 1)) << extra_bits;
                    offset + extra as usize + 1
                };
                assert_eq!(
                    decode_distance(&mut reader, &mut budget, prefix),
                    Ok(expected),
                    "prefix={prefix}, extra={extra}"
                );
            }
        }
        let mut reader = BitReader::new(&[]);
        let mut budget = WorkBudget::new(1);
        assert_eq!(
            decode_length(&mut reader, &mut budget, 24)
                .unwrap_err()
                .kind(),
            DecodeErrorKind::InvalidBitstream
        );
    }

    #[test]
    fn prefix_reader_is_lsb_first_and_budgeted() {
        let mut writer = BitWriter::new();
        writer.write_bits(2, 2).unwrap();
        let bytes = writer.into_bytes();
        let mut reader = BitReader::new(&bytes);
        let mut budget = WorkBudget::new(1);
        assert_eq!(decode_length(&mut reader, &mut budget, 6), Ok(11));
        assert_eq!(budget.remaining(), 0);
        assert_eq!(
            decode_length(&mut reader, &mut budget, 0)
                .unwrap_err()
                .kind(),
            DecodeErrorKind::LimitExceeded
        );
    }

    #[test]
    fn plane_distance_and_linear_distance_follow_spec() {
        // Independent scan-line distances for all 120 plane codes at width
        // 32, transcribed from the normative VP8L distance-code table.
        let expected = [
            32, 1, 33, 31, 64, 2, 65, 63, 34, 30, 66, 62, 96, 3, 97, 95, 35, 29, 98, 94, 67, 61,
            128, 4, 129, 127, 36, 28, 99, 93, 130, 126, 68, 60, 160, 131, 125, 100, 92, 5, 161,
            159, 37, 27, 162, 158, 69, 59, 132, 124, 163, 157, 101, 91, 192, 6, 193, 191, 38, 26,
            194, 190, 70, 58, 164, 156, 133, 123, 195, 189, 102, 90, 224, 7, 225, 223, 165, 155,
            39, 25, 196, 188, 134, 122, 226, 222, 71, 57, 227, 221, 103, 89, 197, 187, 166, 154, 8,
            228, 220, 135, 121, 40, 72, 198, 186, 104, 229, 219, 167, 153, 136, 230, 218, 199, 185,
            168, 231, 217, 200, 232,
        ];
        for (index, expected) in expected.into_iter().enumerate() {
            assert_eq!(
                distance_code_to_distance(index + 1, 32),
                Ok(expected),
                "plane distance code {}",
                index + 1
            );
        }
        assert_eq!(distance_code_to_distance(121, 32), Ok(1));
        assert_eq!(distance_code_to_distance(122, 32), Ok(2));
        assert_eq!(distance_code_to_distance(200, 32), Ok(80));
        assert_eq!(
            distance_code_to_distance(0, 32).unwrap_err().kind(),
            DecodeErrorKind::InvalidBitstream
        );
    }

    fn slow_copy(output: &mut Vec<u32>, length: usize, distance: usize) {
        for _ in 0..length {
            let source = output.len() - distance;
            output.push(output[source]);
        }
    }

    #[test]
    fn optimized_copy_matches_elementwise_reference() {
        for initial_len in 1..=8 {
            for distance in 1..=initial_len {
                for length in 1..=16 {
                    let initial = (0..initial_len)
                        .map(|value| value as u32)
                        .collect::<Vec<_>>();
                    let mut expected = initial.clone();
                    slow_copy(&mut expected, length, distance);
                    let mut actual = initial;
                    let mut budget = WorkBudget::new(length as u64);
                    copy_lz77_pixels(
                        &mut actual,
                        length,
                        distance,
                        initial_len + length,
                        &mut budget,
                    )
                    .unwrap();
                    assert_eq!(
                        actual, expected,
                        "initial={initial_len}, distance={distance}, length={length}"
                    );
                }
            }
        }
    }

    #[test]
    fn copy_validation_accepts_exact_bounds_and_rejects_each_invalid_field() {
        for (produced, length, distance, limit) in [
            (1, 1, 1, 2),
            (8, 1, 8, 9),
            (
                1,
                MAX_BACKWARD_REFERENCE_LENGTH,
                1,
                MAX_BACKWARD_REFERENCE_LENGTH + 1,
            ),
        ] {
            assert_eq!(validate_copy(produced, length, distance, limit), Ok(()));
        }

        for (produced, length, distance, limit) in [
            (1, 0, 1, 1),
            (
                1,
                MAX_BACKWARD_REFERENCE_LENGTH + 1,
                1,
                MAX_BACKWARD_REFERENCE_LENGTH + 2,
            ),
            (1, 1, 0, 2),
            (1, 1, 2, 2),
            (3, 2, 1, 4),
            (usize::MAX, 1, 1, usize::MAX),
        ] {
            assert!(
                validate_copy(produced, length, distance, limit).is_err(),
                "produced={produced}, length={length}, distance={distance}, limit={limit}"
            );
        }
    }

    #[test]
    fn over_budget_copy_does_not_mutate_output() {
        let original = vec![1, 2, 3];
        let mut output = original.clone();
        let mut budget = WorkBudget::new(0);
        assert!(copy_lz77_pixels(&mut output, 1, 1, 4, &mut budget).is_err());
        assert_eq!(output, original);
    }
}
