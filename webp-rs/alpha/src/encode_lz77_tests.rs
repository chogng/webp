use super::*;

#[test]
fn tokenizer_finds_overlapping_repetition() {
    let mut tokens = Vec::new();
    let mut match_table = MatchTable::allocate(15).unwrap();
    walk(b"abcabcabcabcabc", &mut match_table, |token| {
        tokens.push(token);
        Ok::<_, core::convert::Infallible>(())
    })
    .unwrap();
    assert_eq!(
        tokens,
        [
            Token::Literal(b'a'),
            Token::Literal(b'b'),
            Token::Literal(b'c'),
            Token::Copy {
                length: 12,
                distance: 3,
            },
        ]
    );
}
