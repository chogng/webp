use super::{PackedTokenWriter, TokenPacket, packet_for_token, packet_reserve_bytes};
use crate::vp8l::{
    BitWriter, CHANNEL_ALPHABET_SIZE, DISTANCE_ALPHABET_SIZE, EncodeError, EncodingTable,
    EntropyTables, EntropyToken, FIRST_CACHE_SYMBOL, vp8l_prefix,
};

fn reference_append(writer: &mut BitWriter, bits: u64, width: u8) {
    let lower = width.min(32);
    writer.write_bits(bits as u32, lower).unwrap();
    if width > 32 {
        writer.write_bits((bits >> 32) as u32, width - 32).unwrap();
    }
}

fn symbol(writer: &mut BitWriter, table: &EncodingTable, index: usize) {
    let (code, width) = table.codes[index];
    let wire = if width == 0 {
        0
    } else {
        code.reverse_bits() >> (u32::BITS - u32::from(width))
    };
    writer.write_bits(wire, width).unwrap();
}

fn reference_token(writer: &mut BitWriter, token: EntropyToken, tables: &EntropyTables) {
    match token {
        EntropyToken::Cache(index) => symbol(writer, &tables.green, FIRST_CACHE_SYMBOL + index),
        EntropyToken::Literal(rgba) => {
            symbol(writer, &tables.green, usize::from(rgba[1]));
            symbol(writer, &tables.red, usize::from(rgba[0]));
            symbol(writer, &tables.blue, usize::from(rgba[2]));
            symbol(writer, &tables.alpha, usize::from(rgba[3]));
        }
        EntropyToken::Copy {
            length,
            distance_code,
        } => {
            let (length_prefix, length_extra) = vp8l_prefix(length, 24).unwrap();
            symbol(writer, &tables.green, CHANNEL_ALPHABET_SIZE + length_prefix);
            writer.write_bits(length_extra.0, length_extra.1).unwrap();
            let (distance_prefix, distance_extra) =
                vp8l_prefix(distance_code, DISTANCE_ALPHABET_SIZE).unwrap();
            symbol(writer, &tables.distance, distance_prefix);
            writer
                .write_bits(distance_extra.0, distance_extra.1)
                .unwrap();
        }
    }
}

fn synthetic_tables(width: u8) -> EntropyTables {
    let table = |length: usize| EncodingTable {
        codes: vec![(0x6d25, width); length],
    };
    EntropyTables {
        green: table(FIRST_CACHE_SYMBOL + 16),
        red: table(256),
        blue: table(256),
        alpha: table(256),
        distance: table(DISTANCE_ALPHABET_SIZE),
    }
}

#[test]
fn append_matches_reference_for_every_offset_and_width() {
    for offset in 0..8 {
        for width in 0..=64 {
            let mut prefix = BitWriter::new();
            prefix.write_bits(0x55, offset).unwrap();
            let bits = 0xd6a5_39c7_81ef_240b_u64.rotate_left(u32::from(width));
            let mut reference = prefix.clone();
            reference_append(&mut reference, bits, width);
            let prefix_bits = prefix.bit_len();
            let mut candidate = PackedTokenWriter::from_parts(
                prefix.into_bytes(),
                prefix_bits,
                width.div_ceil(8) as usize + 4,
            )
            .unwrap();
            candidate.append(TokenPacket { bits, width }).unwrap();
            assert_eq!(candidate.finish().unwrap(), reference.as_bytes());
        }
    }
}

#[test]
fn append_matches_reference_across_repeated_32_and_64_bit_boundaries() {
    let widths = [0, 1, 7, 8, 15, 31, 32, 33, 58, 60, 63, 64];
    for offset in 0..8 {
        let mut prefix = BitWriter::new();
        prefix.write_bits(0x3d, offset).unwrap();
        let mut reference = prefix.clone();
        let prefix_bits = prefix.bit_len();
        let mut candidate = PackedTokenWriter::from_parts(
            prefix.into_bytes(),
            prefix_bits,
            widths.len() * 9 * 8 + 1,
        )
        .unwrap();
        for repeat in 0..(widths.len() * 9) {
            let width = widths[repeat % widths.len()];
            let bits = 0x517c_c1b7_2722_0a95_u64.wrapping_mul(repeat as u64 + 1);
            reference_append(&mut reference, bits, width);
            candidate.append(TokenPacket { bits, width }).unwrap();
        }
        assert_eq!(candidate.finish().unwrap(), reference.as_bytes());
    }
}

#[test]
fn token_packets_match_reference_at_15_bit_legal_extremes() {
    let tables = synthetic_tables(15);
    for (token, expected_width) in [
        (EntropyToken::Literal([1, 2, 3, 4]), 60),
        (
            EntropyToken::Copy {
                length: 4096,
                distance_code: 121,
            },
            45,
        ),
        (EntropyToken::Cache(3), 15),
    ] {
        assert_eq!(
            packet_for_token(token, &tables).unwrap().width,
            expected_width
        );
        for offset in 0..8 {
            let mut prefix = BitWriter::new();
            prefix.write_bits(0x55, offset).unwrap();
            let mut reference = prefix.clone();
            reference_token(&mut reference, token, &tables);
            let prefix_bits = prefix.bit_len();
            let mut candidate = PackedTokenWriter::from_parts(
                prefix.into_bytes(),
                prefix_bits,
                size_of::<u64>() + 1,
            )
            .unwrap();
            candidate.write_token(token, &tables).unwrap();
            assert_eq!(candidate.finish().unwrap(), reference.as_bytes());
        }
    }
}

#[test]
fn general_vp8l_copy_packet_width_limit_is_58_bits() {
    let mut packet = TokenPacket::new();
    for (value, width) in [(0x1234, 15), (0x155, 10), (0x2345, 15), (0x2_aaaa, 18)] {
        packet.push_wire(value, width).unwrap();
    }
    assert_eq!(packet.width, 58);
}

#[test]
fn finish_preserves_partial_tail_and_zero_padding() {
    for width in 0..=64 {
        let mut reference = BitWriter::new();
        reference.write_bits(5, 3).unwrap();
        reference_append(&mut reference, u64::MAX, width);
        let mut prefix = BitWriter::new();
        prefix.write_bits(5, 3).unwrap();
        let prefix_bits = prefix.bit_len();
        let mut candidate =
            PackedTokenWriter::from_parts(prefix.into_bytes(), prefix_bits, size_of::<u64>() + 1)
                .unwrap();
        candidate
            .append(TokenPacket {
                bits: u64::MAX,
                width,
            })
            .unwrap();
        assert_eq!(candidate.finish().unwrap(), reference.as_bytes());
    }
}

#[test]
fn reserve_and_capacity_failures_are_reported() {
    assert_eq!(
        packet_reserve_bytes(usize::MAX),
        Err(EncodeError::SizeOverflow)
    );
    assert_eq!(
        PackedTokenWriter::from_parts(Vec::new(), 1, 0)
            .err()
            .unwrap(),
        EncodeError::SizeOverflow
    );
    assert_eq!(
        PackedTokenWriter::from_parts(vec![0], 0, 0).err().unwrap(),
        EncodeError::SizeOverflow
    );

    let mut writer = PackedTokenWriter::from_parts(Vec::new(), 0, 0).unwrap();
    assert_eq!(
        writer
            .append(TokenPacket {
                bits: u64::MAX,
                width: 64,
            })
            .unwrap_err(),
        EncodeError::AllocationFailed
    );
}

#[test]
fn malformed_packet_or_table_width_is_rejected() {
    let mut packet = TokenPacket::new();
    packet.push_wire(u32::MAX, 32).unwrap();
    packet.push_wire(u32::MAX, 32).unwrap();
    assert_eq!(packet.push_wire(1, 1), Err(EncodeError::SizeOverflow));
    assert_eq!(
        TokenPacket::new().push_wire(0, 33),
        Err(EncodeError::SizeOverflow)
    );

    let mut tables = synthetic_tables(15);
    tables.green.codes[2] = (0, 33);
    assert!(matches!(
        packet_for_token(EntropyToken::Literal([1, 2, 3, 4]), &tables),
        Err(EncodeError::SizeOverflow)
    ));
    assert!(matches!(
        packet_for_token(EntropyToken::Cache(usize::MAX), &tables),
        Err(EncodeError::SizeOverflow)
    ));
}
