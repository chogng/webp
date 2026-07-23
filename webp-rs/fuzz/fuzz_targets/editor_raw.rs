#![no_main]
#![forbid(unsafe_code)]

use libfuzzer_sys::fuzz_target;
use webp_demux::CompatibilityProfile;
use webp_demux::ContainerLimits;
use webp_demux::DemuxOptions;
use webp_mux::AnimationFrameInput;
use webp_mux::Editor;
use webp_mux::FramePayload;
use webp_mux::MuxChunk;

fuzz_target!(|bytes: &[u8]| {
    let options = DemuxOptions {
        profile: CompatibilityProfile::LibwebpCompatible,
        limits: ContainerLimits {
            max_input_bytes: 1 << 20,
            max_width: 4096,
            max_height: 4096,
            max_pixels: 16 * 1024 * 1024,
            max_metadata_bytes: 1 << 20,
            ..ContainerLimits::default()
        },
    };
    if let Ok(mut editor) = Editor::parse(bytes, &options) {
        match bytes.first().copied().unwrap_or_default() % 8 {
            0 => {
                let _ = editor.set_iccp(vec![1, 2]);
            }
            1 => {
                let _ = editor.set_exif(vec![3]);
            }
            2 => {
                let _ = editor.remove_xmp();
            }
            3 => {
                let index = usize::from(bytes.get(1).copied().unwrap_or_default())
                    % (editor.chunks().len() + 1);
                let _ = editor.insert_chunk(index, MuxChunk::new(*b"fUzZ", vec![4, 5]));
            }
            4 => {
                if !editor.chunks().is_empty() {
                    let index = usize::from(bytes.get(1).copied().unwrap_or_default())
                        % editor.chunks().len();
                    let _ = editor.remove_chunk(index);
                }
            }
            5 => {
                let _ = editor.set_static_vp8l(1, 1, vec![0x2f, 0, 0, 0, 0], false);
            }
            6 => {
                let _ = editor.set_animation(1, 1, [0; 4], 0).and_then(|editor| {
                    editor.add_animation_frame(AnimationFrameInput {
                        x: 0,
                        y: 0,
                        width: 1,
                        height: 1,
                        duration_ms: 1,
                        dispose_to_background: false,
                        blend: true,
                        alpha: None,
                        payload: FramePayload::Vp8l(&[0x2f, 0, 0, 0, 0]),
                    })
                });
            }
            _ => {
                let _ = editor.set_canvas_size(1, 1);
            }
        }
        if let Ok(output) = editor.finish() {
            let _ = webp_demux::Demuxer::parse(&output, &DemuxOptions::default());
        }
    }
});
