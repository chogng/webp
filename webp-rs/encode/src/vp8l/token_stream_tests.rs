use super::*;

#[test]
fn canonical_stream_owns_geometry_copy_census_and_distance_symbols() {
    let rgba = [7, 11, 19, 127].repeat(300);
    let stream =
        TokenStream::collect(&rgba, 300, false, false, 0).expect("collect repeated stream");

    assert_eq!(
        stream.geometry(),
        TokenGeometry {
            width: 300,
            height: 1,
            pixels: 300,
        }
    );
    assert_eq!(
        stream.tokens(),
        [
            EntropyToken::Literal([7, 11, 19, 127]),
            EntropyToken::Copy { length: 299 },
        ]
    );
    let census = stream.statistics().census();
    assert_eq!(census.literal_tokens(), 1);
    assert_eq!(census.cache_tokens(), 0);
    assert_eq!(census.copy_tokens(), 1);
    assert_eq!(census.copied_pixels(), 299);
    assert_eq!(census.distance_symbols(), 1);
}

#[test]
fn canonical_stream_counts_cache_hits_without_changing_token_order() {
    let repeated = [1, 2, 3, 4];
    let repeated_hash = color_cache_index(pack_argb(repeated), 4);
    let separator = (0_u8..=u8::MAX)
        .map(|green| [5, green, 7, 8])
        .find(|pixel| color_cache_index(pack_argb(*pixel), 4) != repeated_hash)
        .expect("find a distinct cache slot");
    let rgba = [repeated, separator, repeated].concat();
    let stream = TokenStream::collect(&rgba, 3, false, false, 4).expect("collect cached stream");

    assert!(matches!(
        stream.tokens(),
        [
            EntropyToken::Literal(first),
            EntropyToken::Literal(second),
            EntropyToken::Cache(_),
        ] if *first == repeated && *second == separator
    ));
    let census = stream.statistics().census();
    assert_eq!(census.literal_tokens(), 2);
    assert_eq!(census.cache_tokens(), 1);
    assert_eq!(census.copy_tokens(), 0);
    assert_eq!(census.copied_pixels(), 0);
    assert_eq!(census.distance_symbols(), 0);
}

#[test]
fn canonical_stream_rejects_incoherent_private_geometry_and_cache_limits() {
    assert_eq!(
        TokenStream::collect(&[1, 2, 3, 4], 0, false, false, 0).err(),
        Some(EncodeError::SizeOverflow)
    );
    assert_eq!(
        TokenStream::collect(&[1, 2, 3], 1, false, false, 0).err(),
        Some(EncodeError::SizeOverflow)
    );
    assert_eq!(
        TokenStream::collect(
            &[1, 2, 3, 4],
            1,
            false,
            false,
            MAX_ENCODER_COLOR_CACHE_BITS + 1,
        )
        .err(),
        Some(EncodeError::SizeOverflow)
    );
}
