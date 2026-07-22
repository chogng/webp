use webp_decode::DecodeLimits;
use webp_decode::DecodeOptions;
use webp_decode::Metadata;
use webp_decode::decode;
use webp_decode::read_metadata;
use webp_encode::encode_lossless_rgba_with_metadata;

#[test]
fn lossless_encoder_muxes_all_static_metadata_combinations() {
    let rgba = [
        1, 2, 3, 255, 10, 20, 30, 128, 40, 50, 60, 0, 70, 80, 90, 255,
    ];
    for mask in 0_u8..8 {
        let metadata = Metadata {
            iccp: (mask & 1 != 0).then_some(vec![0, 1, 2]),
            exif: (mask & 2 != 0).then_some(vec![3, 4, 5, 6]),
            xmp: (mask & 4 != 0).then_some(b"<xmp/>".to_vec()),
        };
        let encoded = encode_lossless_rgba_with_metadata(2, 2, &rgba, &metadata)
            .expect("encode metadata WebP");
        let decoded = decode(&encoded, &DecodeOptions::default()).expect("decode metadata WebP");
        assert_eq!(decoded.rgba, rgba, "mask {mask}");
        assert_eq!(
            read_metadata(&encoded, &DecodeLimits::default()).expect("read encoded metadata"),
            metadata,
            "mask {mask}"
        );
        if mask == 0 {
            assert!(!encoded.windows(4).any(|window| window == b"VP8X"));
        } else {
            assert_eq!(&encoded[12..16], b"VP8X");
        }
    }
}
