use std::path::PathBuf;

use webp_testkit::verify_clic_validation;

#[test]
fn local_clic_validation_corpus_is_rust_consumable() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("third_party/benchdata/clic");
    if !root.join("validation-manifest.json").is_file() {
        return;
    }
    let summary = verify_clic_validation(root).expect("CLIC manifest and images must verify");
    assert_eq!(summary.images, 102, "official CLIC validation image count");
    assert!(
        summary.bytes > 0,
        "CLIC validation corpus must not be empty"
    );
}
