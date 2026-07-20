use std::path::PathBuf;

use webp::{read_info, DecodeLimits};
use webp_testkit::{FixtureApi, FixtureClass, FixtureRunner};

#[test]
fn generated_animation_corpus_is_readable_by_rust() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("third_party/corpus/animation-v1");
    if !root.is_dir() {
        return;
    }

    let summary = FixtureRunner::with_fixture_root(root.join("manifests"), &root)
        .run_all(|fixture, bytes| {
            assert_eq!(fixture.class, FixtureClass::MustAccept);
            assert_eq!(fixture.api, FixtureApi::ReadInfo);
            let info = read_info(bytes, &DecodeLimits::default())
                .unwrap_or_else(|error| panic!("{}: {error}", fixture.id));
            assert_eq!(Some(info.width), fixture.expected_width);
            assert_eq!(Some(info.height), fixture.expected_height);
            assert!(info.is_animated, "{} must be animated", fixture.id);
            Ok::<_, String>(())
        })
        .expect("animation manifests and inputs must be valid");

    assert!(summary.fixtures > 0, "animation corpus must not be empty");
}
