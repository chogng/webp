use webp_container::AnimationFrameInput;
use webp_container::DemuxOptions;
use webp_container::Editor;
use webp_container::FramePayload;
use webp_container::MuxChunk;
use webp_container::Muxer;

#[test]
fn muxer_constructs_a_static_container_with_metadata_and_extensions() {
    let mut muxer = Muxer::static_vp8l(2, 3, vec![1, 2, 3], false).unwrap();
    muxer.set_exif(vec![4, 5]).unwrap();
    muxer
        .add_chunk(MuxChunk::new(*b"uNk!", vec![9, 8, 7]))
        .unwrap();
    let bytes = muxer.finish().unwrap();
    let parsed = webp_container::Demuxer::parse(&bytes, &DemuxOptions::default()).unwrap();

    assert_eq!(parsed.metadata().exif, Some(&[4, 5][..]));
    assert_eq!(
        parsed.image().unwrap().bitstream(),
        webp_container::ImageBitstream::Vp8l(&[1, 2, 3])
    );
    assert!(
        parsed
            .unknown_chunks()
            .any(|chunk| chunk.payload == [9, 8, 7])
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
            payload: FramePayload::Vp8(&[1]),
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
                    payload: FramePayload::Vp8(&[3]),
                },
            )
            .unwrap()
    );
    let output = editor.finish().unwrap();
    let parsed = webp_container::Demuxer::parse(&output, &DemuxOptions::default()).unwrap();
    let frame = parsed.animation().unwrap().frame(0).unwrap();

    assert_eq!(frame.duration_ms, 7);
    assert_eq!(frame.alpha, Some(&[2][..]));
    assert_eq!(frame.bitstream, webp_container::FrameBitstream::Vp8(&[3]));
    assert_eq!(parsed.chunk(2).unwrap().fourcc, webp_container::ANMF);
}
