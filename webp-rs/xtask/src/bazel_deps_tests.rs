use super::render;
use std::fs;
use std::time::SystemTime;

#[test]
fn renders_sorted_workspace_path_dependencies() {
    let unique = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("system clock follows Unix epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("webp-bazel-deps-{unique}"));
    fs::create_dir_all(root.join("alpha")).expect("create alpha package");
    fs::create_dir_all(root.join("core")).expect("create core package");
    fs::write(
        root.join("Cargo.toml"),
        "[workspace]\nmembers = [\"alpha\", \"core\"]\nresolver = \"2\"\n",
    )
    .expect("write workspace manifest");
    fs::write(
        root.join("alpha/Cargo.toml"),
        "[package]\nname = \"webp-alpha\"\nversion = \"0.1.0\"\n\
         [dependencies]\nwebp-core = { path = \"../core\" }\n",
    )
    .expect("write alpha manifest");
    fs::write(
        root.join("core/Cargo.toml"),
        "[package]\nname = \"webp-core\"\nversion = \"0.1.0\"\n",
    )
    .expect("write core manifest");

    let generated = render(&root).expect("render Bazel dependencies");
    assert!(generated.contains("\"webp-rs/alpha\": [\n        \"//webp-rs/core\",\n    ],"));

    fs::remove_dir_all(root).expect("remove temporary workspace");
}
