use std::path::PathBuf;

use webp_testkit::{FixtureApi, FixtureClass, FixtureRunner};

#[test]
fn reference_encoder_corpus_is_rust_consumable() {
    let corpus_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("third_party/corpus");
    let mut fixtures = 0;
    for name in ["reference-v1", "reference-edge-v1"] {
        let root = corpus_root.join(name);
        if !root.is_dir() {
            continue;
        }

        let summary = FixtureRunner::with_fixture_root(root.join("manifests"), &root)
            .run_all(|fixture, bytes| {
                assert!(matches!(fixture.class, FixtureClass::MustAccept));
                assert!(matches!(fixture.api, FixtureApi::Decode));
                assert!(!bytes.is_empty(), "{} must contain WebP bytes", fixture.id);
                Ok::<_, String>(())
            })
            .expect("reference corpus manifests and bytes must be valid");
        fixtures += summary.fixtures;
    }

    assert!(fixtures > 0, "reference corpus must not be empty");
}
