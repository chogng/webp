#![cfg(feature = "animation")]
//! Optional per-frame differential test against libwebp's WebPAnimDecoder.

use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use webp_decode::AnimationDecoder;
use webp_decode::AnimationDecoderOptions;

#[test]
fn streaming_frames_match_libwebp_anim_decoder() {
    let Some(prefix) = std::env::var_os("LIBWEBP_ANIM_ORACLE_PREFIX").map(PathBuf::from) else {
        eprintln!("skip animation decoder oracle: set LIBWEBP_ANIM_ORACLE_PREFIX");
        return;
    };
    let Some(corpus) = corpus_root() else {
        eprintln!("skip animation decoder oracle: animation corpus is unavailable");
        return;
    };
    let scratch = ScratchDirectory::new("animation-decoder-oracle");
    let helper = scratch.0.join("libwebp-animation-oracle");
    compile_helper(&prefix, &helper);
    for name in [
        "two-frame-loop.webp",
        "animation-blend-loop-one.webp",
        "animation-dispose-no-blend-loop-zero.webp",
    ] {
        let input = corpus.join(name);
        let reference = scratch.0.join(format!("{name}.frames"));
        let output = Command::new(&helper)
            .arg(&input)
            .arg(&reference)
            .output()
            .expect("run libwebp animation decoder helper");
        assert!(
            output.status.success(),
            "libwebp animation decoder failed for {name}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let input_bytes =
            fs::read(&input).unwrap_or_else(|error| panic!("read {}: {error}", input.display()));
        let mut decoder = AnimationDecoder::new(&input_bytes, AnimationDecoderOptions::default())
            .unwrap_or_else(|error| panic!("Rust decoder rejected {name}: {error}"));
        let frame_bytes = decoder.info().width as usize * decoder.info().height as usize * 4;
        let reference = fs::read(&reference).expect("read libwebp animation output");
        let mut offset = 0;
        while let Some(frame) = decoder
            .next_frame()
            .unwrap_or_else(|error| panic!("Rust decoder failed for {name}: {error}"))
        {
            let timestamp = u32::from_le_bytes(
                reference
                    .get(offset..offset + 4)
                    .unwrap_or_else(|| panic!("libwebp omitted a timestamp for {name}"))
                    .try_into()
                    .expect("four-byte timestamp"),
            );
            offset += 4;
            let expected = reference
                .get(offset..offset + frame_bytes)
                .unwrap_or_else(|| panic!("libwebp omitted pixels for {name}"));
            offset += frame_bytes;
            assert_eq!(
                frame.timestamp_ms,
                u64::from(timestamp),
                "timestamp for {name}"
            );
            assert_eq!(frame.pixels, expected, "RGBA canvas for {name}");
        }
        assert_eq!(
            offset,
            reference.len(),
            "libwebp emitted extra frames for {name}"
        );
    }
}

fn corpus_root() -> Option<PathBuf> {
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let local = repository.join("third_party/corpus/animation-v1");
    local.is_dir().then_some(local)
}

fn compile_helper(prefix: &Path, output: &Path) {
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let source = repository.join("tools/libwebp-animation-oracle.c");
    let result = Command::new("cc")
        .arg("-std=c99")
        .arg("-I")
        .arg(prefix.join("include"))
        .arg(&source)
        .arg("-L")
        .arg(prefix.join("lib"))
        .args(["-lwebpdemux", "-lwebp", "-o"])
        .arg(output)
        .output()
        .expect("compile libwebp animation decoder helper");
    assert!(
        result.status.success(),
        "compile libwebp animation decoder helper: {}",
        String::from_utf8_lossy(&result.stderr)
    );
}

struct ScratchDirectory(PathBuf);

impl ScratchDirectory {
    fn new(label: &str) -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock is after Unix epoch")
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("webp-{label}-{}-{unique}", std::process::id()));
        fs::create_dir_all(&path).expect("create animation oracle scratch directory");
        Self(path)
    }
}

impl Drop for ScratchDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
