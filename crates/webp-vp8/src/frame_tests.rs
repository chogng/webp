use super::*;
use crate::test_support::{
    TestBoolWriter, key_frame, write_coefficient_updates, write_quantization_header,
};
use crate::{MacroblockPixels, Vp8Header, parse_riff_payload};
use webp_core::{DecodeErrorKind, DecodeLimits};

#[test]
fn macroblock_storage_exposes_reconstructed_edges() {
    let frame = Vp8Header {
        width: 17,
        height: 17,
        version: 0,
        first_partition_len: 0,
        horizontal_scale: 0,
        vertical_scale: 0,
    };
    let mut image = Vp8YuvImage::new(&frame, &DecodeLimits::default()).unwrap();
    let pixels = MacroblockPixels {
        y: std::array::from_fn(|index| index as u8),
        u: std::array::from_fn(|index| (index + 64) as u8),
        v: std::array::from_fn(|index| (index + 128) as u8),
    };
    image.store_macroblock(0, 0, pixels);
    let right_edges = image.edges(1, 0);
    assert_eq!(
        right_edges.left_y.unwrap(),
        std::array::from_fn(|row| (row * 16 + 15) as u8)
    );
    assert_eq!(
        right_edges.left_u.unwrap(),
        std::array::from_fn(|row| (row * 8 + 71) as u8)
    );
    assert_eq!(
        right_edges.left_v.unwrap(),
        std::array::from_fn(|row| (row * 8 + 135) as u8)
    );
    let below_edges = image.edges(0, 1);
    assert_eq!(below_edges.top_y.unwrap(), pixels.y[240..256]);
    assert_eq!(below_edges.top_u.unwrap(), pixels.u[56..64]);
    assert_eq!(below_edges.top_v.unwrap(), pixels.v[56..64]);
}

#[test]
fn macroblock_storage_enforces_allocation_limit() {
    let frame = Vp8Header {
        width: 1,
        height: 1,
        version: 0,
        first_partition_len: 0,
        horizontal_scale: 0,
        vertical_scale: 0,
    };
    let limits = DecodeLimits {
        max_alloc_bytes: 383,
        ..DecodeLimits::default()
    };
    assert_eq!(
        Vp8YuvImage::new(&frame, &limits).unwrap_err().kind(),
        DecodeErrorKind::LimitExceeded
    );
}

#[test]
fn yuv_image_converts_visible_rectangle_to_vp8_rgba() {
    let image = Vp8YuvImage {
        width: 2,
        height: 2,
        y_stride: 2,
        uv_stride: 1,
        y: vec![16, 235, 81, 145],
        u: vec![128],
        v: vec![128],
    };
    assert_eq!(
        image.to_rgba(&DecodeLimits::default()).unwrap(),
        vec![
            0, 0, 0, 255, 255, 255, 255, 255, 76, 76, 76, 255, 150, 150, 150, 255
        ]
    );
}

#[test]
fn yuv_image_rejects_short_visible_plane() {
    let image = Vp8YuvImage {
        width: 2,
        height: 2,
        y_stride: 2,
        uv_stride: 1,
        y: vec![0; 3],
        u: vec![128],
        v: vec![128],
    };
    assert_eq!(
        image.to_rgba(&DecodeLimits::default()).unwrap_err().kind(),
        DecodeErrorKind::InvalidParameter
    );
}

#[test]
fn intra_frame_decoder_reconstructs_a_skipped_macroblock() {
    let mut writer = TestBoolWriter::new();
    writer.write_bool(false, 128); // colour space
    writer.write_bool(false, 128); // clamp type
    writer.write_bool(false, 128); // no segmentation
    writer.write_bool(false, 128); // normal filter
    writer.write_literal(0, 6); // filter level
    writer.write_literal(0, 3); // filter sharpness
    writer.write_bool(false, 128); // no filter deltas
    writer.write_literal(0, 2); // one token partition
    write_quantization_header(&mut writer, 0, [0; 5], false);
    write_coefficient_updates(&mut writer, &[], true, 1);
    writer.write_bool(true, 1); // skip residuals
    writer.write_bool(true, 145); // 16x16 luma
    writer.write_bool(false, 156); // DC luma
    writer.write_bool(false, 163);
    writer.write_bool(false, 142); // DC chroma
    let partition_zero = writer.finish();
    let mut payload = key_frame(1, 1, 0, true, partition_zero.len() as u32).to_vec();
    payload.extend_from_slice(&partition_zero);
    payload.push(0); // Non-empty final token partition, never consumed by skip.

    let limits = DecodeLimits::default();
    let frame = parse_riff_payload(&payload, None, &limits).unwrap();
    let image = decode_intra_frame(&payload, &frame, &limits).unwrap();
    assert_eq!(image.width, 1);
    assert_eq!(image.height, 1);
    assert_eq!(image.y_stride, 16);
    assert_eq!(image.uv_stride, 8);
    assert_eq!(image.y.len(), 16 * 16);
    assert_eq!(image.u.len(), 8 * 8);
    assert!(image.y.iter().all(|&sample| sample == 128));
    assert!(image.u.iter().all(|&sample| sample == 128));
    assert!(image.v.iter().all(|&sample| sample == 128));
}
