#![no_main]
#![forbid(unsafe_code)]

use libfuzzer_sys::fuzz_target;
use webp_core::BitReader;
use webp_vp8l_huffman::read_huffman_code;

const MAX_ALPHABET_SIZE: usize = 2_328;

fuzz_target!(|input: &[u8]| {
    let Some((&low, rest)) = input.split_first() else {
        return;
    };
    let Some((&high, encoded_code)) = rest.split_first() else {
        return;
    };
    let requested = usize::from(u16::from_le_bytes([low, high]));
    let alphabet_size = requested % MAX_ALPHABET_SIZE + 1;
    let mut bits = BitReader::new(encoded_code);
    let _ = read_huffman_code(&mut bits, alphabet_size);
});
