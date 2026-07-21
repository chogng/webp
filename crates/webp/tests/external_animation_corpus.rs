use std::path::PathBuf;

use webp::{DecodeLimits, DecodeOptions, decode_animation, read_info};

fn corpus_root() -> Option<PathBuf> {
    let cargo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("third_party/corpus/animation-v1");
    match std::env::var_os("TEST_SRCDIR") {
        Some(runfiles) => {
            let workspace = std::env::var_os("TEST_WORKSPACE").unwrap_or_else(|| "_main".into());
            let root = PathBuf::from(runfiles)
                .join(workspace)
                .join("third_party/corpus/animation-v1");
            assert!(
                root.is_dir(),
                "Bazel external-corpus test requires the fetched animation corpus"
            );
            Some(root)
        }
        None if cargo_root.is_dir() => Some(cargo_root),
        None => None,
    }
}

fn pixel(rgba: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
    let index = ((y * width + x) * 4) as usize;
    rgba[index..index + 4].try_into().unwrap()
}

#[test]
fn generated_animation_corpus_is_readable_by_rust() {
    let Some(root) = corpus_root() else { return };

    let path = root.join("two-frame-loop.webp");
    let bytes =
        std::fs::read(&path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    let info = read_info(&bytes, &DecodeLimits::default())
        .unwrap_or_else(|error| panic!("two-frame-loop.webp: {error}"));
    assert_eq!((info.width, info.height), (128, 128));
    assert!(info.is_animated, "two-frame-loop.webp must be animated");
}

#[test]
fn animation_state_vectors_match_composed_pixel_oracles() {
    let Some(root) = corpus_root() else { return };
    let options = DecodeOptions::default();

    let blend = std::fs::read(root.join("animation-blend-loop-one.webp")).unwrap();
    let blend = decode_animation(&blend, &options).unwrap();
    assert_eq!((blend.width, blend.height, blend.loop_count), (128, 96, 1));
    assert_eq!(
        blend
            .frames
            .iter()
            .map(|frame| frame.duration_ms)
            .collect::<Vec<_>>(),
        [100, 40]
    );
    assert_eq!(pixel(&blend.frames[0].rgba, 128, 0, 0), [220, 35, 35, 255]);
    assert_eq!(
        pixel(&blend.frames[1].rgba, 128, 31, 24),
        [220, 35, 35, 255]
    );
    assert_eq!(
        pixel(&blend.frames[1].rgba, 128, 32, 24),
        [35, 35, 220, 255]
    );

    let dispose = std::fs::read(root.join("animation-dispose-no-blend-loop-zero.webp")).unwrap();
    let dispose = decode_animation(&dispose, &options).unwrap();
    assert_eq!(
        (dispose.width, dispose.height, dispose.loop_count),
        (128, 96, 0)
    );
    assert_eq!(
        dispose
            .frames
            .iter()
            .map(|frame| frame.duration_ms)
            .collect::<Vec<_>>(),
        [100, 40, 80]
    );
    assert_eq!(
        pixel(&dispose.frames[1].rgba, 128, 32, 24),
        [35, 35, 220, 255]
    );
    assert_eq!(
        pixel(&dispose.frames[2].rgba, 128, 32, 24),
        [10, 20, 30, 255]
    );
    assert_eq!(
        pixel(&dispose.frames[2].rgba, 128, 48, 24),
        [35, 220, 35, 255]
    );
}
