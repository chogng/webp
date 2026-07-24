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
            EntropyToken::Copy {
                length: 299,
                distance_code: 121,
            },
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
fn coarse_spatial_statistics_match_aggregated_fine_blocks() {
    let width = 137;
    let height = 151;
    let mut rgba = Vec::with_capacity(width * height * 4);
    for index in 0..width * height {
        rgba.extend_from_slice(&[index as u8, (index >> 3) as u8, (index >> 7) as u8, u8::MAX]);
    }
    let stream = TokenStream::collect(&rgba, width, false, false, 0).expect("collect stream");
    let (_, aggregated) = stream
        .spatial_statistics_pair(32, 128)
        .expect("aggregate fine statistics");
    let direct = stream
        .spatial_statistics(128)
        .expect("collect coarse statistics");
    assert_eq!(aggregated.block_width(), direct.block_width());
    assert_eq!(aggregated.blocks(), direct.blocks());
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

#[test]
fn streaming_tokenization_and_cache_selection_match_materialized_reference() {
    for width in [1_usize, 2, 3, 5, 17] {
        for height in [1_usize, 2, 4, 9] {
            let mut state = (width as u32)
                .wrapping_mul(0x9e37_79b9)
                .wrapping_add(height as u32);
            let mut rgba = Vec::with_capacity(width * height * 4);
            for index in 0..width * height {
                state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                let repeated = index > 0 && index % 7 >= 3;
                if repeated {
                    let previous: [u8; 4] = rgba[(index - 1) * 4..index * 4]
                        .try_into()
                        .expect("complete preceding pixel");
                    rgba.extend_from_slice(&previous);
                } else {
                    rgba.extend_from_slice(&[
                        state as u8,
                        (state >> 8) as u8,
                        (state >> 16) as u8,
                        (state >> 24) as u8,
                    ]);
                }
            }
            for use_subtract_green in [false, true] {
                for use_left_predictor in [false, true] {
                    assert_eq!(
                        select_color_cache_bits(
                            &rgba,
                            width,
                            use_subtract_green,
                            use_left_predictor,
                        ),
                        materialized_cache_selection(
                            &rgba,
                            width,
                            use_subtract_green,
                            use_left_predictor,
                        ),
                    );
                    for color_cache_bits in 0..=MAX_ENCODER_COLOR_CACHE_BITS {
                        let stream = TokenStream::collect(
                            &rgba,
                            width,
                            use_subtract_green,
                            use_left_predictor,
                            color_cache_bits,
                        )
                        .expect("collect streaming tokens");
                        assert_eq!(
                            stream.tokens(),
                            materialized_tokens(
                                &rgba,
                                width,
                                use_subtract_green,
                                use_left_predictor,
                                color_cache_bits,
                            ),
                            "{width}x{height}, subtract={use_subtract_green}, \
                             predictor={use_left_predictor}, cache={color_cache_bits}",
                        );
                    }
                }
            }
        }
    }
}

fn materialized_cache_selection(
    rgba: &[u8],
    width: usize,
    use_subtract_green: bool,
    use_left_predictor: bool,
) -> u8 {
    let mut selected_bits = 0;
    let mut best_hits = 0_u32;
    for bits in 1..=MAX_ENCODER_COLOR_CACHE_BITS {
        let mut cache = [0_u32; MAX_COLOR_CACHE_SIZE];
        let mut hits = 0_u32;
        for index in 0..rgba.len() / 4 {
            let color = pack_argb(residual_at(
                rgba,
                index,
                width,
                use_subtract_green,
                use_left_predictor,
                None,
            ));
            let cache_index = color_cache_index(color, bits);
            if cache[cache_index] == color {
                hits = hits.saturating_add(1);
            }
            cache[cache_index] = color;
        }
        if hits > best_hits {
            best_hits = hits;
            selected_bits = bits;
        }
    }
    selected_bits
}

fn materialized_tokens(
    rgba: &[u8],
    width: usize,
    use_subtract_green: bool,
    use_left_predictor: bool,
    color_cache_bits: u8,
) -> Vec<EntropyToken> {
    let residuals = (0..rgba.len() / 4)
        .map(|index| {
            residual_at(
                rgba,
                index,
                width,
                use_subtract_green,
                use_left_predictor,
                None,
            )
        })
        .collect::<Vec<_>>();
    let mut tokens = Vec::new();
    let mut color_cache = [0_u32; MAX_COLOR_CACHE_SIZE];
    let mut index = 0_usize;
    while index < residuals.len() {
        let residual = residuals[index];
        if index != 0 && residual == residuals[index - 1] {
            let mut length = 1_usize;
            while length < 4096
                && index + length < residuals.len()
                && residuals[index + length] == residual
            {
                length += 1;
            }
            if length >= 3 {
                tokens.push(EntropyToken::Copy {
                    length,
                    distance_code: 121,
                });
                for _ in 0..length {
                    update_color_cache(&mut color_cache, color_cache_bits, pack_argb(residual));
                }
                index += length;
                continue;
            }
        }
        let color = pack_argb(residual);
        let cache_index = if color_cache_bits == 0 {
            0
        } else {
            color_cache_index(color, color_cache_bits)
        };
        if color_cache_bits != 0 && color_cache[cache_index] == color {
            tokens.push(EntropyToken::Cache(cache_index));
        } else {
            tokens.push(EntropyToken::Literal(residual));
        }
        color_cache[cache_index] = color;
        index += 1;
    }
    tokens
}
