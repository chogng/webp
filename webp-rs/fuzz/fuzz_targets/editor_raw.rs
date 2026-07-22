#![no_main]
#![forbid(unsafe_code)]

use libfuzzer_sys::fuzz_target;
use webp_demux::CompatibilityProfile;
use webp_demux::ContainerLimits;
use webp_demux::DemuxOptions;
use webp_mux::Editor;

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
        match bytes.first().copied().unwrap_or_default() % 3 {
            0 => {
                let _ = editor.set_iccp(vec![1, 2]);
            }
            1 => {
                let _ = editor.set_exif(vec![3]);
            }
            _ => {
                let _ = editor.remove_xmp();
            }
        }
        if let Ok(output) = editor.finish() {
            let _ = webp_demux::Demuxer::parse(&output, &DemuxOptions::default());
        }
    }
});
