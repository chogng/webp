#![forbid(unsafe_code)]
//! VP8L entropy-stream primitives.
//!
//! This crate deliberately contains no Huffman-table builder yet.  It provides
//! the spec-defined length/distance prefix expansion and the bounds-checked
//! LZ77 copy operation that the entropy decoder will call after decoding its
//! Huffman symbols.

use webp_core::{BitReader, DecodeError, DecodeErrorKind, WorkBudget};

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
    validate_copy(output.len(), length, distance, output_limit)?;
    budget.consume(u64::try_from(length).map_err(|_| {
        DecodeError::new(
            DecodeErrorKind::LimitExceeded,
            None,
            "copy length does not fit work counter",
        )
    })?)?;
    output.try_reserve(length).map_err(|_| {
        DecodeError::new(
            DecodeErrorKind::AllocationFailed,
            None,
            "LZ77 output allocation failed",
        )
    })?;

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
            assert!(decode_with_extra(prefix, 0, DISTANCE_PREFIX_COUNT) >= 1);
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
        assert_eq!(distance_code_to_distance(1, 10), Ok(10));
        assert_eq!(distance_code_to_distance(2, 10), Ok(1));
        assert_eq!(distance_code_to_distance(3, 10), Ok(11));
        assert_eq!(distance_code_to_distance(4, 10), Ok(9));
        assert_eq!(distance_code_to_distance(121, 10), Ok(1));
        assert_eq!(
            distance_code_to_distance(0, 10).unwrap_err().kind(),
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
    fn invalid_or_over_budget_copy_does_not_mutate_output() {
        let original = vec![1, 2, 3];
        for (length, distance, limit, budget_units) in
            [(1, 0, 4, 1), (1, 4, 4, 1), (2, 1, 4, 1), (1, 1, 4, 0)]
        {
            let mut output = original.clone();
            let mut budget = WorkBudget::new(budget_units);
            assert!(copy_lz77_pixels(&mut output, length, distance, limit, &mut budget).is_err());
            assert_eq!(output, original);
        }
    }
}
