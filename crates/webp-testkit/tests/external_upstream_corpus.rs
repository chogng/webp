use std::path::PathBuf;

use webp_testkit::{FixtureApi, FixtureClass, FixtureRunner};

#[test]
fn selected_upstream_vectors_are_rust_consumable() {
    let cargo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("third_party/corpus/libwebp-test-data");
    let root = match std::env::var_os("TEST_SRCDIR") {
        Some(runfiles) => {
            let workspace = std::env::var_os("TEST_WORKSPACE").unwrap_or_else(|| "_main".into());
            let root = PathBuf::from(runfiles)
                .join(workspace)
                .join("third_party/corpus/libwebp-test-data");
            assert!(
                root.is_dir(),
                "Bazel external-corpus test requires the fetched libwebp corpus"
            );
            root
        }
        None if cargo_root.is_dir() => cargo_root,
        None => return,
    };

    let summary = FixtureRunner::with_fixture_root(root.join("manifests"), &root)
        .run_all(|fixture, bytes| {
            assert!(!bytes.is_empty(), "{} must contain WebP bytes", fixture.id);
            if fixture.class == FixtureClass::MustAccept {
                assert_eq!(fixture.api, FixtureApi::Decode);
                assert_eq!(fixture.codec, webp_testkit::Codec::Vp8l);
                assert!(fixture.expected_width.is_some());
                assert!(fixture.expected_height.is_some());
                assert!(fixture.expected_rgba_sha256.is_some());
            }
            Ok::<_, String>(())
        })
        .expect("selected upstream manifests and bytes must be valid");

    assert_eq!(summary.fixtures, 68, "selected upstream corpus size");
}
