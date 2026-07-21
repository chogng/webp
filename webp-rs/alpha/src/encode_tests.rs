use super::*;
use crate::decode;
use crate::parse_header;
use webp_core::CompatibilityProfile;
use webp_core::DecodeLimits;

const SAMPLES: [u8; 12] = [0, 1, 2, 255, 17, 9, 200, 4, 88, 88, 99, 3];

fn round_trip(compression: AlphaCompression, filter: AlphaFilter) {
    let options = AlphaEncodeOptions {
        compression,
        filter,
        preprocessing: AlphaPreprocessing::None,
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
        filter: AlphaFilter::Gradient,
        preprocessing: AlphaPreprocessing::LevelReduction,
    };
    let payload = encode(&[37], 1, 1, options).unwrap();
    assert_eq!(
        parse_header(&payload, CompatibilityProfile::SpecStrict).unwrap(),
        crate::AlphaHeader {
            compression: options.compression,
            filter: options.filter,
            preprocessing: options.preprocessing,
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
}
