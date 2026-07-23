use webp_mux::AnimationFrameInput;
use webp_mux::DemuxOptions;
use webp_mux::Editor;
use webp_mux::FramePayload;
use webp_mux::MuxChunk;
use webp_mux::Muxer;

fn vp8l_payload(width: u32, height: u32, has_alpha: bool) -> Vec<u8> {
    let fields = (width - 1) | ((height - 1) << 14) | (u32::from(has_alpha) << 28);
    let mut payload = vec![0x2f];
    payload.extend_from_slice(&fields.to_le_bytes());
    payload
}

fn vp8_payload(width: u16, height: u16) -> Vec<u8> {
    let mut payload = vec![0x10, 0, 0, 0x9d, 0x01, 0x2a];
    payload.extend_from_slice(&width.to_le_bytes());
    payload.extend_from_slice(&height.to_le_bytes());
    payload
}

#[test]
fn muxer_constructs_a_static_container_with_metadata_and_extensions() {
    let image = vp8l_payload(2, 3, false);
    let mut muxer = Muxer::static_vp8l(2, 3, image.clone(), false).unwrap();
    muxer.set_exif(vec![4, 5]).unwrap();
    muxer
        .add_chunk(MuxChunk::new(*b"uNk!", vec![9, 8, 7]))
        .unwrap();
    let bytes = muxer.finish().unwrap();
    let parsed = webp_demux::Demuxer::parse(&bytes, &DemuxOptions::default()).unwrap();

    assert_eq!(parsed.metadata().exif, Some(&[4, 5][..]));
    assert_eq!(
        parsed.image().unwrap().bitstream(),
        webp_demux::ImageBitstream::Vp8l(&image)
    );
    assert!(
        parsed
            .unknown_chunks()
            .any(|chunk| chunk.payload == [9, 8, 7])
    );
}

#[test]
fn muxer_supports_indexed_chunk_mutation() {
    let mut muxer = Muxer::static_vp8l(1, 1, vp8l_payload(1, 1, false), false).unwrap();
    muxer.set_xmp(vec![9]).unwrap();
    assert!(muxer.remove_xmp().unwrap());
    muxer
        .insert_chunk(1, MuxChunk::new(*b"one!", vec![1]))
        .unwrap();
    muxer
        .insert_chunk(2, MuxChunk::new(*b"two!", vec![2]))
        .unwrap();
    assert_eq!(muxer.chunks()[1].fourcc(), *b"one!");
    assert_eq!(muxer.remove_chunk(1).unwrap().fourcc(), *b"one!");
    assert_eq!(muxer.chunks()[1].fourcc(), *b"two!");

    let bytes = muxer.finish().unwrap();
    let parsed = webp_demux::Demuxer::parse(&bytes, &DemuxOptions::default()).unwrap();
    assert!(
        parsed
            .unknown_chunks()
            .any(|chunk| chunk.fourcc == *b"two!")
    );
}

#[test]
fn editor_replaces_an_animation_frame_without_codec_reencoding() {
    let mut muxer = Muxer::animation(2, 2, [0; 4], 0).unwrap();
    muxer
        .add_animation_frame(AnimationFrameInput {
            x: 0,
            y: 0,
            width: 2,
            height: 2,
            duration_ms: 1,
            dispose_to_background: false,
            blend: true,
            alpha: None,
            payload: FramePayload::Vp8(&vp8_payload(2, 2)),
        })
        .unwrap();
    let mut editor = Editor::parse(&muxer.finish().unwrap(), &DemuxOptions::default()).unwrap();

    assert!(
        editor
            .replace_animation_frame(
                0,
                AnimationFrameInput {
                    x: 0,
                    y: 0,
                    width: 2,
                    height: 2,
                    duration_ms: 7,
                    dispose_to_background: false,
                    blend: true,
                    alpha: Some(&[2]),
                    payload: FramePayload::Vp8(&vp8_payload(2, 2)),
                },
            )
            .unwrap()
    );
    let output = editor.finish().unwrap();
    let parsed = webp_demux::Demuxer::parse(&output, &DemuxOptions::default()).unwrap();
    let frame = parsed.animation().unwrap().frame(0).unwrap();

    assert_eq!(frame.duration_ms, 7);
    assert_eq!(frame.alpha, Some(&[2][..]));
    assert_eq!(
        frame.bitstream,
        webp_demux::FrameBitstream::Vp8(&vp8_payload(2, 2))
    );
    assert_eq!(parsed.chunk(2).unwrap().fourcc, webp_container::ANMF);
}

#[test]
fn vp8l_animation_alpha_hint_updates_vp8x() {
    let payload = vp8l_payload(2, 2, true);
    let mut muxer = Muxer::animation(2, 2, [0; 4], 0).unwrap();
    muxer
        .add_animation_frame(AnimationFrameInput {
            x: 0,
            y: 0,
            width: 2,
            height: 2,
            duration_ms: 1,
            dispose_to_background: false,
            blend: true,
            alpha: None,
            payload: FramePayload::Vp8l(&payload),
        })
        .unwrap();

    let bytes = muxer.finish().unwrap();
    let parsed = webp_demux::Demuxer::parse(&bytes, &DemuxOptions::default()).unwrap();

    assert!(parsed.vp8x().unwrap().flags.alpha());
}

#[test]
fn editor_inserts_removes_and_reconfigures_animation_frames() {
    let payload = vp8l_payload(2, 2, false);
    let mut muxer = Muxer::animation(4, 2, [0; 4], 0).unwrap();
    muxer
        .add_animation_frame(AnimationFrameInput {
            x: 2,
            y: 0,
            width: 2,
            height: 2,
            duration_ms: 10,
            dispose_to_background: false,
            blend: true,
            alpha: None,
            payload: FramePayload::Vp8l(&payload),
        })
        .unwrap();
    let mut editor = Editor::parse(&muxer.finish().unwrap(), &DemuxOptions::default()).unwrap();
    editor
        .insert_animation_frame(
            0,
            AnimationFrameInput {
                x: 0,
                y: 0,
                width: 2,
                height: 2,
                duration_ms: 5,
                dispose_to_background: false,
                blend: true,
                alpha: None,
                payload: FramePayload::Vp8l(&payload),
            },
        )
        .unwrap();
    assert!(editor.remove_animation_frame(1).is_some());
    editor.set_animation_params([1, 2, 3, 4], 7).unwrap();
    editor.set_canvas_size(2, 2).unwrap();

    let bytes = editor.finish().unwrap();
    let parsed = webp_demux::Demuxer::parse(&bytes, &DemuxOptions::default()).unwrap();
    let animation = parsed.animation().unwrap();

    assert_eq!(animation.frame_count(), 1);
    assert_eq!(animation.frame(0).unwrap().duration_ms, 5);
    assert_eq!(animation.background_bgra, [3, 2, 1, 4]);
    assert_eq!(animation.loop_count, 7);
    assert_eq!(parsed.vp8x().unwrap().canvas_width, 2);
}

#[test]
fn editor_converts_image_kind_and_preserves_non_image_chunks() {
    let mut muxer = Muxer::static_vp8l(2, 2, vp8l_payload(2, 2, false), false).unwrap();
    muxer.set_exif(vec![1, 2]).unwrap();
    muxer
        .add_chunk(MuxChunk::new(*b"uNk!", vec![3, 4]))
        .unwrap();
    let mut editor = Editor::parse(&muxer.finish().unwrap(), &DemuxOptions::default()).unwrap();
    editor.set_animation(2, 2, [0; 4], 2).unwrap();
    editor
        .add_animation_frame(AnimationFrameInput {
            x: 0,
            y: 0,
            width: 2,
            height: 2,
            duration_ms: 1,
            dispose_to_background: false,
            blend: true,
            alpha: None,
            payload: FramePayload::Vp8l(&vp8l_payload(2, 2, false)),
        })
        .unwrap();
    editor
        .set_static_vp8l(2, 2, vp8l_payload(2, 2, false), false)
        .unwrap();

    let bytes = editor.finish().unwrap();
    let parsed = webp_demux::Demuxer::parse(&bytes, &DemuxOptions::default()).unwrap();

    assert!(parsed.animation().is_none());
    assert!(parsed.image().is_some());
    assert_eq!(parsed.metadata().exif, Some(&[1, 2][..]));
    assert!(
        parsed
            .unknown_chunks()
            .any(|chunk| chunk.fourcc == *b"uNk!")
    );
}
