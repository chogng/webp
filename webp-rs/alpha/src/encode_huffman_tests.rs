use super::*;

#[test]
fn frequency_tree_gives_hot_symbols_shorter_codes() {
    let lengths = code_lengths(&[1_000, 20, 10, 1]).unwrap();
    assert_eq!(lengths, [1, 2, 3, 3]);
}

#[test]
fn empty_and_single_symbol_alphabets_remain_zero_bit_streams() {
    assert_eq!(code_lengths(&[0; 40]).unwrap()[0], 1);
    assert_eq!(code_lengths(&[0, 0, 7, 0]).unwrap(), [0, 0, 1, 0]);
}

#[test]
fn code_length_runs_use_vp8l_repeat_symbols() {
    assert_eq!(
        encode_code_lengths(&[0; 150]).unwrap(),
        [
            CodeLengthToken::repeat(18, 127, 7).unwrap(),
            CodeLengthToken::repeat(18, 1, 7).unwrap(),
        ]
    );
    assert_eq!(
        encode_code_lengths(&[4; 8]).unwrap(),
        [
            CodeLengthToken::value(4),
            CodeLengthToken::repeat(16, 3, 2).unwrap(),
            CodeLengthToken::value(4),
        ]
    );
}
