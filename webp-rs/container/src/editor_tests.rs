use super::*;
use crate::CompatibilityProfile;
use crate::ContainerErrorKind;
use crate::FramePayload;
use crate::Muxer;

#[test]
fn unchanged_editor_round_trips_strict_bytes_and_unknown_chunks() {
    let bytes = Muxer::static_vp8l(2, 3, vec![1, 2, 3], false)
        .unwrap()
        .with_chunk(MuxChunk::new(*b"uNk!", vec![9, 8, 7]))
        .unwrap()
        .finish()
        .unwrap();
    let edited = Editor::parse(&bytes, &DemuxOptions::default())
        .unwrap()
        .finish()
        .unwrap();

    assert_eq!(edited, bytes);
}

#[test]
fn metadata_edits_preserve_codec_and_unknown_payloads() {
    let source = Muxer::static_vp8l(2, 3, vec![1, 2, 3], false)
        .unwrap()
        .with_chunk(MuxChunk::new(*b"uNk!", vec![9, 8, 7]))
        .unwrap()
        .finish()
        .unwrap();
    let mut editor = Editor::parse(&source, &DemuxOptions::default()).unwrap();
    editor.set_exif(vec![4, 5]).unwrap();
    editor.set_xmp(vec![6]).unwrap();
    let edited = editor.finish().unwrap();
    let parsed = crate::Demuxer::parse(&edited, &DemuxOptions::default()).unwrap();

    assert_eq!(parsed.metadata().exif, Some(&[4, 5][..]));
    assert_eq!(parsed.metadata().xmp, Some(&[6][..]));
    assert_eq!(parsed.chunk(1).unwrap().payload, &[1, 2, 3]);
    assert!(
        parsed
            .unknown_chunks()
            .any(|chunk| chunk.payload == [9, 8, 7])
    );
}

#[test]
fn metadata_edits_require_an_extended_container() {
    let source = crate::serialize_vp8l(vec![1], 0, 0, false, Metadata::default()).unwrap();
    let mut editor = Editor::parse(&source, &DemuxOptions::default()).unwrap();

    assert_eq!(
        editor.set_exif(vec![1]).unwrap_err().kind(),
        ContainerErrorKind::InvalidContainer
    );
}

#[test]
fn removing_metadata_clears_its_vp8x_flag() {
    let mut muxer = Muxer::static_vp8l(2, 3, vec![1], false).unwrap();
    muxer.set_xmp(vec![9]).unwrap();
    let source = muxer.finish().unwrap();
    let mut editor = Editor::parse(&source, &DemuxOptions::default()).unwrap();

    assert!(editor.remove_xmp().unwrap());
    let output = editor.finish().unwrap();
    let parsed = crate::Demuxer::parse(&output, &DemuxOptions::default()).unwrap();

    assert_eq!(parsed.metadata().xmp, None);
    assert!(!parsed.vp8x().unwrap().flags.xmp());
}

#[test]
fn frame_edits_keep_animation_container_valid() {
    let mut muxer = Muxer::animation(2, 2, [1, 2, 3, 4], 3).unwrap();
    muxer
        .add_animation_frame(AnimationFrameInput {
            x: 0,
            y: 0,
            width: 2,
            height: 2,
            duration_ms: 5,
            dispose_to_background: false,
            blend: true,
            alpha: None,
            payload: FramePayload::Vp8l(&[1, 2]),
        })
        .unwrap();
    let source = muxer.finish().unwrap();
    let mut editor = Editor::parse(&source, &DemuxOptions::default()).unwrap();
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
                    dispose_to_background: true,
                    blend: false,
                    alpha: None,
                    payload: FramePayload::Vp8l(&[3]),
                },
            )
            .unwrap()
    );
    let output = editor.finish().unwrap();
    let parsed = crate::parse(
        &output,
        CompatibilityProfile::SpecStrict,
        &crate::ContainerLimits::default(),
    )
    .unwrap();

    assert_eq!(parsed.animation().unwrap().frame(0).unwrap().duration_ms, 7);
}
