use super::*;

#[test]
fn restricted_subimages_use_lz77_without_a_colour_cache() {
    let rgba = [17, 29, 43, 255].repeat(64);
    let stream = collect_restricted_stream(&rgba, 64).expect("collect restricted stream");

    assert_eq!(stream.color_cache_bits(), 0);
    assert!(
        stream.statistics().census().copy_tokens() > 0,
        "a repeated transform image must retain its LZ77/RLE references"
    );
}

#[test]
fn restricted_subimage_writer_emits_a_single_group_prefix_and_tables() {
    let rgba = [3, 5, 7, 255].repeat(8);
    let mut bits = BitWriter::new();
    write_restricted_entropy_image(&mut bits, &rgba, 8).expect("write restricted subimage");

    assert_eq!(
        bits.as_bytes()[0] & 1,
        0,
        "the prefix disables colour cache"
    );
    assert!(
        bits.bit_len() > 1,
        "the no-cache flag is followed by five tables"
    );
}
