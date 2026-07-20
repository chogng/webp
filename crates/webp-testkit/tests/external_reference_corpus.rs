use std::path::PathBuf;

use webp_testkit::{FixtureApi, FixtureClass, FixtureRunner};

#[test]
fn reference_encoder_corpus_is_rust_consumable() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("third_party/corpus/reference-v1");
    if !root.is_dir() {
        // The corpus is deliberately ignored by Git. CI jobs that provision it
        // execute this same test; ordinary contributor test runs stay offline.
        return;
    }

    let summary = FixtureRunner::with_fixture_root(root.join("manifests"), &root)
        .run_all(|fixture, bytes| {
            assert!(matches!(fixture.class, FixtureClass::MustAccept));
            assert!(matches!(fixture.api, FixtureApi::Decode));
            assert!(!bytes.is_empty(), "{} must contain WebP bytes", fixture.id);
            Ok::<_, String>(())
        })
        .expect("reference corpus manifests and bytes must be valid");

    assert!(summary.fixtures > 0, "reference corpus must not be empty");
}
