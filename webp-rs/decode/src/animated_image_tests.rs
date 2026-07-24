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
        [10, 20, 30, 255, 0, 0, 0, 0, 0, 0, 0, 0]
    );
    assert_eq!(animation.frames[1].duration_ms, 20);
    assert_eq!(
        animation.frames[1].rgba,
        [0, 0, 0, 0, 0, 0, 0, 0, 100, 0, 0, 255]
    );
}

#[test]
fn stateful_decoder_fetches_frames_and_restarts_from_the_first() {
    let data = two_frame_animation();
    let mut decoder = AnimationDecoder::new(&data, AnimationDecoderOptions::default()).unwrap();
    assert_eq!(
        *decoder.info(),
        AnimationInfo {
            width: 3,
            height: 1,
            loop_count: 0,
            background_rgba: [2, 3, 4, 1],
            frame_count: 2,
        }
    );
    assert!(decoder.has_more_frames());

    let first = {
        let frame = decoder.next_frame().unwrap().unwrap();
        assert_eq!(frame.duration_ms, 10);
        assert_eq!(frame.timestamp_ms, 10);
        assert_eq!(frame.color_mode, AnimationColorMode::Rgba);
        frame.pixels.to_vec()
    };
    let second = {
        let frame = decoder.next_frame().unwrap().unwrap();
        assert_eq!(frame.duration_ms, 20);
        assert_eq!(frame.timestamp_ms, 30);
        frame.pixels.to_vec()
    };
    assert!(!decoder.has_more_frames());
    assert!(decoder.next_frame().unwrap().is_none());

    decoder.reset();
    assert!(decoder.has_more_frames());
    let replay = {
        let frame = decoder.next_frame().unwrap().unwrap();
        assert_eq!(frame.timestamp_ms, 10);
        frame.pixels.to_vec()
    };
    assert_eq!(replay, first);
    assert_ne!(first, second);
}

#[test]
fn stateful_decoder_converts_the_borrowed_canvas_to_requested_color_mode() {
    let data = two_frame_animation();
    let mut decoder = AnimationDecoder::new(
        &data,
        AnimationDecoderOptions {
            color_mode: AnimationColorMode::BgraPremultiplied,
            ..AnimationDecoderOptions::default()
        },
    )
    .unwrap();
    assert_eq!(decoder.demuxer().frame_count(), 2);
    let frame = decoder.next_frame().unwrap().unwrap();
    assert_eq!(frame.color_mode, AnimationColorMode::BgraPremultiplied);
    assert_eq!(&frame.pixels[..4], [30, 20, 10, 255]);
    assert_eq!(&frame.pixels[4..8], [0, 0, 0, 0]);
}

#[test]
fn threaded_frame_decode_matches_serial_alpha_composition() {
    let bitstream = vp8l_pixel([10, 20, 30, 255]);
    let alpha = [0, 77];
    let frame = webp_demux::AnimationFrame {
        x: 0,
        y: 0,
        width: 1,
        height: 1,
        duration_ms: 1,
        dispose_to_background: false,
        blend: false,
        alpha: Some(&alpha),
        bitstream: FrameBitstream::Vp8l(&bitstream),
    };
    let options = DecodeOptions::default();
    let serial = decode_animation_frame(&frame, &options, false).unwrap();
    let threaded = decode_animation_frame(&frame, &options, true).unwrap();
    assert_eq!(serial, [10, 20, 30, 77]);
    assert_eq!(threaded, serial);
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
