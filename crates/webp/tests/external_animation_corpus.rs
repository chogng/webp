use std::path::PathBuf;

use webp::{DecodeLimits, read_info};

#[test]
fn generated_animation_corpus_is_readable_by_rust() {
    let cargo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("third_party/corpus/animation-v1");
    let root = match std::env::var_os("TEST_SRCDIR") {
        Some(runfiles) => {
            let workspace = std::env::var_os("TEST_WORKSPACE").unwrap_or_else(|| "_main".into());
            let root = PathBuf::from(runfiles)
                .join(workspace)
                .join("third_party/corpus/animation-v1");
            assert!(
                root.is_dir(),
                "Bazel external-corpus test requires the fetched animation corpus"
            );
            root
        }
        None if cargo_root.is_dir() => cargo_root,
        None => return,
    };

    let path = root.join("two-frame-loop.webp");
    let bytes =
        std::fs::read(&path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    let info = read_info(&bytes, &DecodeLimits::default())
        .unwrap_or_else(|error| panic!("two-frame-loop.webp: {error}"));
    assert_eq!((info.width, info.height), (128, 128));
    assert!(info.is_animated, "two-frame-loop.webp must be animated");
}
