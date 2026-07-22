//! Tests for complete alpha payload writing.

use super::*;
use crate::LossyEncodeOptions;
use crate::encode_lossy_rgba_with_alpha_options;
use webp_container::AlphaCompression;
use webp_container::AlphaFilter;
use webp_container::AlphaHeader;
use webp_container::AlphaPreprocessing;
use webp_decode::DecodeOptions;
use webp_decode::decode;

#[cfg(feature = "alpha-benchmark-internals")]
use crate::alpha::BenchmarkWriterVariant;
#[cfg(feature = "alpha-benchmark-internals")]
use crate::alpha::set_benchmark_writer_variant;

const SAMPLES: [u8; 12] = [0, 1, 2, 255, 17, 9, 200, 4, 88, 88, 99, 3];

fn round_trip(compression: AlphaCompression, filter: AlphaFilter) {
    let options = AlphaEncodeOptions {
        compression,
        filter: filter.into(),
        quality: 100,
    };
    let rgba = SAMPLES
        .iter()
        .flat_map(|&alpha| [17, 34, 51, alpha])
        .collect::<Vec<_>>();
    let encoded = encode_lossy_rgba_with_alpha_options(
        4,
        3,
        &rgba,
        LossyEncodeOptions { quality: 75 },
        options,
    )
    .expect("encode alpha plane");
    assert_eq!(
        decode(&encoded, &DecodeOptions::default())
            .expect("decode encoded alpha plane")
            .rgba
            .chunks_exact(4)
            .map(|pixel| pixel[3])
            .collect::<Vec<_>>(),
        SAMPLES,
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
        AlphaHeader {
            compression: options.compression,
            filter: AlphaFilter::Gradient,
            preprocessing: AlphaPreprocessing::LevelReduction,
        }
        .to_byte(),
        payload[0],
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
        payload[0] & 0b11,
        AlphaHeader {
            compression: AlphaCompression::Raw,
            filter: AlphaFilter::None,
            preprocessing: AlphaPreprocessing::None,
        }
        .to_byte()
            & 0b11
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
    assert_eq!(payload[0] & 0b11, 1, "payload uses lossless ALPH");
    assert_eq!(payload.len(), smallest_fixed);
}

#[cfg(feature = "alpha-benchmark-internals")]
#[test]
fn benchmark_writer_controls_preserve_complete_payload_bytes() {
    let samples = (0..4096)
        .map(|index| ((index / 11 + index / 64) % 29) as u8)
        .collect::<Vec<_>>();
    let options = AlphaEncodeOptions {
        compression: AlphaCompression::Lossless,
        filter: AlphaFilterSelection::Fast,
        quality: 100,
    };
    set_benchmark_writer_variant(BenchmarkWriterVariant::Reference);
    let reference = encode(&samples, 64, 64, options).unwrap();
    set_benchmark_writer_variant(BenchmarkWriterVariant::PacketReference);
    let packet_reference = encode(&samples, 64, 64, options).unwrap();
    set_benchmark_writer_variant(BenchmarkWriterVariant::Packed);
    let packed = encode(&samples, 64, 64, options).unwrap();
    assert_eq!(packet_reference, reference);
    assert_eq!(packed, reference);
}
