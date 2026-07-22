use super::*;
use crate::BitWriter;

fn push_chunk(output: &mut Vec<u8>, fourcc: [u8; 4], payload: &[u8]) {
    output.extend_from_slice(&fourcc);
    output.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    output.extend_from_slice(payload);
    if payload.len() % 2 == 1 {
        output.push(0);
    }
}

fn vp8l_pixel(rgba: [u8; 4]) -> Vec<u8> {
    let mut writer = BitWriter::new();
    writer.write_bits(0x2f, 8).unwrap();
    writer.write_bits(0, 14).unwrap(); // 1px width
    writer.write_bits(0, 14).unwrap(); // 1px height
    writer.write_bits(u32::from(rgba[3] != 255), 1).unwrap();
    writer.write_bits(0, 3).unwrap(); // version
    writer.write_bits(0, 3).unwrap(); // transform/cache/meta flags
    for channel in [rgba[1], rgba[0], rgba[2], rgba[3], 0] {
        writer.write_bits(1, 1).unwrap(); // simple code
        writer.write_bits(0, 1).unwrap(); // one symbol
        writer.write_bits(1, 1).unwrap(); // 8-bit symbol id
        writer.write_bits(u32::from(channel), 8).unwrap();
    }
    writer.into_bytes()
}

fn anmf(x: u32, width: u32, duration_ms: u32, flags: u8, pixel: [u8; 4]) -> Vec<u8> {
    let mut frame = Vec::new();
    frame.extend_from_slice(&(x / 2).to_le_bytes()[..3]);
    frame.extend_from_slice(&[0; 3]);
    frame.extend_from_slice(&(width - 1).to_le_bytes()[..3]);
    frame.extend_from_slice(&[0; 3]);
    frame.extend_from_slice(&duration_ms.to_le_bytes()[..3]);
    frame.push(flags);
    let payload = vp8l_pixel(pixel);
    push_chunk(&mut frame, webp_container::VP8L, &payload);
    frame
}

fn two_frame_animation() -> Vec<u8> {
    let mut body = b"WEBP".to_vec();
    let vp8x = [0b0000_0010, 0, 0, 0, 2, 0, 0, 0, 0, 0]; // 3x1 canvas
    push_chunk(&mut body, webp_container::VP8X, &vp8x);
    push_chunk(&mut body, webp_container::ANIM, &[4, 3, 2, 1, 0, 0]);
    push_chunk(
        &mut body,
        webp_container::ANMF,
        &anmf(0, 1, 10, 1, [10, 20, 30, 255]),
    );
    push_chunk(
        &mut body,
        webp_container::ANMF,
        &anmf(2, 1, 20, 0, [100, 0, 0, 255]),
    );
    let mut riff = b"RIFF".to_vec();
    riff.extend_from_slice(&(body.len() as u32).to_le_bytes());
    riff.extend_from_slice(&body);
    riff
}

#[test]
fn animated_vp8l_frames_are_composed_in_display_order() {
    let animation = decode_animation(&two_frame_animation(), &DecodeOptions::default()).unwrap();
    assert_eq!(
        (animation.width, animation.height, animation.loop_count),
        (3, 1, 0)
    );
    assert_eq!(animation.frames.len(), 2);
    assert_eq!(animation.frames[0].duration_ms, 10);
    assert_eq!(
        animation.frames[0].rgba,
        [10, 20, 30, 255, 2, 3, 4, 1, 2, 3, 4, 1]
    );
    assert_eq!(animation.frames[1].duration_ms, 20);
    assert_eq!(
        animation.frames[1].rgba,
        [2, 3, 4, 1, 2, 3, 4, 1, 100, 0, 0, 255]
    );
}

#[test]
fn animation_decode_rejects_static_images() {
    let static_image = vp8l_pixel([0; 4]);
    let mut body = b"WEBP".to_vec();
    push_chunk(&mut body, webp_container::VP8L, &static_image);
    let mut riff = b"RIFF".to_vec();
    riff.extend_from_slice(&(body.len() as u32).to_le_bytes());
    riff.extend_from_slice(&body);
    assert_eq!(
        decode_animation(&riff, &DecodeOptions::default())
            .unwrap_err()
            .kind(),
        DecodeErrorKind::UnsupportedFeature
    );
}
