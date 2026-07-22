use super::*;

#[test]
fn u24_round_trip_preserves_every_wire_bit() {
    for value in [0, 1, 0x12_34_56, 0xff_ff_ff] {
        assert_eq!(read_u24_le(write_u24_le(value)), value);
    }
}

#[test]
fn bit_writer_appends_lsb_first_and_zero_pads() {
    let mut writer = BitWriter::new();
    writer.write_bits(0b101, 3).unwrap();
    writer.write_bits(0b11, 2).unwrap();
    assert_eq!(writer.bit_len(), 5);
    assert_eq!(writer.as_bytes(), &[0b0001_1101]);
}

#[test]
fn bit_writer_rejects_widths_above_word_size() {
    let mut writer = BitWriter::new();
    assert_eq!(writer.write_bits(0, 33), Err(BitWriteError::InvalidWidth));
}
