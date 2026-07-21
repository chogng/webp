use super::*;
use crate::AlphaFilter;
use crate::decode;
use crate::parse_header;
use webp_core::CompatibilityProfile;
use webp_core::DecodeLimits;

const SAMPLES: [u8; 12] = [0, 1, 2, 255, 17, 9, 200, 4, 88, 88, 99, 3];

fn round_trip(compression: AlphaCompression, filter: AlphaFilter) {
    let options = AlphaEncodeOptions {
        compression,
        filter: filter.into(),
        quality: 100,
    };
    let payload = encode(&SAMPLES, 4, 3, options).expect("encode alpha plane");
    assert_eq!(
        decode(
            &payload,
            4,
            3,
            CompatibilityProfile::SpecStrict,
            &DecodeLimits::default(),
        )
        .expect("decode encoded alpha plane"),
        SAMPLES
    );
}

#[test]
fn every_compression_and_filter_round_trips() {
    for compression in [AlphaCompression::Raw, AlphaCompression::Lossless] {
        for filter in [
            AlphaFilter::None,
            AlphaFilter::Horizontal,
            AlphaFilter::Vertical,
            AlphaFilter::Gradient,
        ] {
            round_trip(compression, filter);
        }
    }
}

#[test]
fn header_serialization_preserves_every_field() {
    let options = AlphaEncodeOptions {
        compression: AlphaCompression::Lossless,
        filter: AlphaFilter::Gradient.into(),
        quality: 80,
    };
    let payload = encode(&vec![37; 64 * 64], 64, 64, options).unwrap();
    assert_eq!(
        parse_header(&payload, CompatibilityProfile::SpecStrict).unwrap(),
        crate::AlphaHeader {
            compression: options.compression,
            filter: AlphaFilter::Gradient,
            preprocessing: AlphaPreprocessing::LevelReduction,
        }
    );
}

#[test]
fn validates_dimensions_and_sample_length() {
    assert_eq!(
        encode(&[], 0, 1, AlphaEncodeOptions::default()),
        Err(AlphaEncodeError::InvalidDimensions)
    );
    assert_eq!(
        encode(&[1], 2, 1, AlphaEncodeOptions::default()),
        Err(AlphaEncodeError::InvalidSampleLength)
    );
    assert_eq!(
        encode(
            &[],
            MAX_LOSSLESS_DIMENSION + 1,
            1,
            AlphaEncodeOptions {
                compression: AlphaCompression::Lossless,
                ..AlphaEncodeOptions::default()
            },
        ),
        Err(AlphaEncodeError::InvalidDimensions)
    );
    assert_eq!(
        encode(
            &[1],
            1,
            1,
            AlphaEncodeOptions {
                quality: 101,
                ..AlphaEncodeOptions::default()
            }
        ),
        Err(AlphaEncodeError::InvalidQuality)
    );
}

#[test]
fn lossless_falls_back_to_raw_when_compression_expands() {
    let payload = encode(
        &[37],
        1,
        1,
        AlphaEncodeOptions {
            compression: AlphaCompression::Lossless,
            filter: AlphaFilter::Gradient.into(),
            quality: 100,
        },
    )
    .unwrap();
    assert_eq!(
        parse_header(&payload, CompatibilityProfile::SpecStrict)
            .unwrap()
            .compression,
        AlphaCompression::Raw
    );
}

#[test]
fn best_filter_selects_the_smallest_lossless_payload() {
    let samples = (0..64)
        .flat_map(|_| (0..64).map(|x| (x * 3) as u8))
        .collect::<Vec<_>>();
    let payload = encode(
        &samples,
        64,
        64,
        AlphaEncodeOptions {
            compression: AlphaCompression::Lossless,
            filter: AlphaFilterSelection::Best,
            quality: 100,
        },
    )
    .unwrap();
    let header = parse_header(&payload, CompatibilityProfile::SpecStrict).unwrap();
    let smallest_fixed = [
        AlphaFilter::None,
        AlphaFilter::Horizontal,
        AlphaFilter::Vertical,
        AlphaFilter::Gradient,
    ]
    .into_iter()
    .map(|filter| {
        encode(
            &samples,
            64,
            64,
            AlphaEncodeOptions {
                compression: AlphaCompression::Lossless,
                filter: filter.into(),
                quality: 100,
            },
        )
        .unwrap()
        .len()
    })
    .min()
    .unwrap();
    assert_eq!(header.compression, AlphaCompression::Lossless);
    assert_eq!(payload.len(), smallest_fixed);
}

#[test]
fn lz77_tokenizer_finds_repeated_sequences() {
    let mut tokens = Vec::new();
    let mut match_heads = allocate_match_heads().unwrap();
    walk_tokens(b"abcabcabcabcabc", &mut match_heads, |token| {
        tokens.push(token);
        Ok(())
    })
    .unwrap();
    assert_eq!(
        tokens,
        [
            EntropyToken::Literal(b'a'),
            EntropyToken::Literal(b'b'),
            EntropyToken::Literal(b'c'),
            EntropyToken::Copy {
                length: 12,
                distance: 3,
            },
        ]
    );
}
