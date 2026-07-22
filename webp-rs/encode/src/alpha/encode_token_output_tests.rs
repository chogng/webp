//! Tests for alpha payload token output.

use super::super::AlphaEncodeError;
use super::super::backward_references;
use super::super::backward_references::Token;
use super::{
    MAX_TOKEN_PACKET_BITS, PackedTokenSink, TokenPacket, WriterVariant, append_packet_reference,
    packet_for_token, write_tokens_with_variant,
};
use crate::vp8l::huffman::table_from_codes_for_test;
use webp_utils::BitWriter;

fn packet(bits: u64, width: u8) -> TokenPacket {
    TokenPacket { bits, width }
}

#[test]
fn packed_sink_matches_reference_at_every_offset_and_legal_width() {
    for offset in 0..8 {
        for width in 0..=MAX_TOKEN_PACKET_BITS {
            let mut prefix = BitWriter::new();
            prefix.write_bits(0x55, offset).unwrap();
            let mut reference = prefix.clone();
            let value = 0xd6a5_b4c3_9281_706f_u64;
            append_packet_reference(&mut reference, packet(value, width)).unwrap();
            let mut packed = PackedTokenSink::from_prefix(prefix, 4).unwrap();
            packed.append(packet(value, width)).unwrap();
            assert_eq!(packed.finish().unwrap(), reference.into_bytes());
        }
    }
}

#[test]
fn repeated_word_crossings_and_partial_tail_match_reference() {
    let mut prefix = BitWriter::new();
    prefix.write_bits(5, 3).unwrap();
    let mut reference = prefix.clone();
    let mut packed = PackedTokenSink::from_prefix(prefix, 32).unwrap();
    for width in [58, 0, 1, 31, 32, 33, 57, 9, 58, 2] {
        let value = 0xfedc_ba98_7654_3210;
        append_packet_reference(&mut reference, packet(value, width)).unwrap();
        packed.append(packet(value, width)).unwrap();
    }
    assert_eq!(packed.finish().unwrap(), reference.into_bytes());
}

#[test]
fn rejects_packet_above_proven_legal_maximum() {
    let mut packed = PackedTokenSink::from_prefix(BitWriter::new(), 4).unwrap();
    assert_eq!(
        packed.append(packet(u64::MAX, MAX_TOKEN_PACKET_BITS + 1)),
        Err(AlphaEncodeError::SizeOverflow)
    );
}

#[test]
fn final_tail_is_zero_padded() {
    let mut packed = PackedTokenSink::from_prefix(BitWriter::new(), 1).unwrap();
    packed.append(packet(0b1_1111_1111, 9)).unwrap();
    assert_eq!(packed.finish().unwrap(), [0xff, 0x01]);
}

#[test]
fn literal_and_copy_reach_proven_widths_in_exact_lsb_order() {
    let mut green_codes = vec![(0, 0); 280];
    green_codes[255] = (0x1234, 15);
    green_codes[279] = (0x2345, 15);
    let green = table_from_codes_for_test(green_codes);
    let mut distance_codes = vec![(0, 0); 40];
    distance_codes[39] = (0x3456, 15);
    let distance = table_from_codes_for_test(distance_codes);

    let literal = packet_for_token(&green, &distance, 1, Token::Literal(255)).unwrap();
    assert_eq!(literal.width, 15);

    let copy = packet_for_token(
        &green,
        &distance,
        1,
        Token::Copy {
            length: 4096,
            distance: 1_048_456,
        },
    )
    .unwrap();
    assert_eq!(copy.width, 58);

    let length_wire = 0x2345_u32.reverse_bits() >> 17;
    let distance_wire = 0x3456_u32.reverse_bits() >> 17;
    let expected = u64::from(length_wire)
        | (u64::from(1023_u32) << 15)
        | (u64::from(distance_wire) << 25)
        | (u64::from(262_143_u32) << 40);
    assert_eq!(copy.bits, expected);
}

#[test]
fn zero_width_codes_retain_maximum_extras() {
    let green = table_from_codes_for_test(vec![(0, 0); 280]);
    let distance = table_from_codes_for_test(vec![(0, 0); 40]);
    let copy = packet_for_token(
        &green,
        &distance,
        1,
        Token::Copy {
            length: 4096,
            distance: 1_048_456,
        },
    )
    .unwrap();
    assert_eq!(copy.width, 28);
    assert_eq!(copy.bits, 1023 | (262_143_u64 << 10));
}

#[test]
fn prefix_and_reserve_arithmetic_are_checked() {
    assert_eq!(
        PackedTokenSink::from_parts(vec![0], 0, 0).err(),
        Some(AlphaEncodeError::SizeOverflow)
    );
    assert_eq!(
        PackedTokenSink::from_parts(vec![0], 9, 0).err(),
        Some(AlphaEncodeError::SizeOverflow)
    );
    assert_eq!(
        PackedTokenSink::from_parts(Vec::new(), 0, usize::MAX).err(),
        Some(AlphaEncodeError::SizeOverflow)
    );
}

#[test]
fn bounded_sink_rejects_insufficient_capacity() {
    let mut packed = PackedTokenSink::from_parts(Vec::new(), 0, 0).unwrap();
    assert_eq!(
        packed.append(packet(u64::MAX, 58)),
        Err(AlphaEncodeError::AllocationFailed)
    );
}

#[test]
fn packet_composition_rejects_field_and_total_overflow() {
    let mut value = TokenPacket::new();
    assert_eq!(value.push(0, 33), Err(AlphaEncodeError::SizeOverflow));
    value.push(u32::MAX, 32).unwrap();
    value.push(u32::MAX, 32).unwrap();
    assert_eq!(value.push(1, 1), Err(AlphaEncodeError::SizeOverflow));
}

#[test]
fn reference_packet_and_packed_traversals_match_with_cache_or_replay() {
    let samples = (0..1024)
        .map(|index| ((index / 7) % 17) as u8)
        .collect::<Vec<_>>();
    let green = table_from_codes_for_test(vec![(0, 0); 280]);
    let distance = table_from_codes_for_test(vec![(0, 0); 40]);
    let mut token_table = backward_references::MatchTable::allocate(samples.len()).unwrap();
    let mut cached = Vec::new();
    backward_references::walk(&samples, &mut token_table, |token| {
        cached.push(backward_references::pack(token));
        Ok::<_, ()>(())
    })
    .unwrap();

    for cache in [None, Some(cached.as_slice())] {
        let render = |variant| {
            let mut prefix = BitWriter::new();
            prefix.write_bits(5, 3).unwrap();
            let mut match_table = backward_references::MatchTable::allocate(samples.len()).unwrap();
            write_tokens_with_variant(
                &samples,
                32,
                &mut match_table,
                cache,
                prefix,
                &green,
                &distance,
                variant,
            )
            .unwrap()
        };
        let reference = render(WriterVariant::Reference);
        assert_eq!(render(WriterVariant::PacketReference), reference);
        assert_eq!(render(WriterVariant::Packed), reference);
    }
}
