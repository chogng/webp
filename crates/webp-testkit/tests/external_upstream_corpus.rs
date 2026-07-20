use std::path::PathBuf;

use webp_testkit::{FixtureClass, FixtureRunner};

#[test]
fn selected_upstream_vectors_are_rust_consumable() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("third_party/corpus/libwebp-test-data");
    if !root.is_dir() {
        return;
    }

    let summary = FixtureRunner::with_fixture_root(root.join("manifests"), &root)
        .run_all(|fixture, bytes| {
            assert_eq!(fixture.class, FixtureClass::ImplementationDefined);
            assert!(!bytes.is_empty(), "{} must contain WebP bytes", fixture.id);
            Ok::<_, String>(())
        })
        .expect("selected upstream manifests and bytes must be valid");

    assert_eq!(summary.fixtures, 64, "selected upstream corpus size");
}
