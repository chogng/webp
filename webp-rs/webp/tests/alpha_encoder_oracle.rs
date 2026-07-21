//! Pinned-libwebp oracle coverage for emitted `ALPH` payloads.

use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use webp::AlphaCompression;
use webp::AlphaEncodeOptions;
use webp::AlphaFilterSelection;
use webp::DecodeOptions;
use webp::LossyEncodeOptions;
use webp::decode;
use webp::encode_lossy_rgba_with_alpha_options;

const ALPHA_VECTORS: &[&str] = &[
    "alpha_no_compression.webp",
    "alpha_filter_0_method_0.webp",
    "alpha_filter_1_method_0.webp",
    "alpha_filter_2_method_0.webp",
    "alpha_filter_3_method_0.webp",
    "alpha_filter_0_method_1.webp",
    "alpha_filter_1_method_1.webp",
    "alpha_filter_2_method_1.webp",
    "alpha_filter_3_method_1.webp",
];
static SCRATCH_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[test]
fn lossless_alpha_encoder_matches_pinned_dwebp_on_upstream_vectors() {
    let Some(corpus) = corpus_root() else {
        eprintln!("skip ALPH encoder corpus: fetch libwebp-test-data");
        return;
    };
    let Some(dwebp) = pinned_dwebp() else {
        eprintln!("skip ALPH encoder oracle: fetch pinned libwebp");
        return;
    };
    let scratch = ScratchDirectory::new();
    let mut lossless_payloads = 0_usize;
    for name in ALPHA_VECTORS {
        let source = fs::read(corpus.join(name)).expect("read upstream ALPH vector");
        let image = decode(&source, &DecodeOptions::default()).expect("decode source ALPH vector");
        let encoded = encode_lossy_rgba_with_alpha_options(
            image.width,
            image.height,
            &image.rgba,
            LossyEncodeOptions { quality: 75 },
            AlphaEncodeOptions {
                compression: AlphaCompression::Lossless,
                filter: AlphaFilterSelection::Best,
                quality: 100,
            },
        )
        .expect("encode lossless ALPH vector");
        lossless_payloads += usize::from(encoded_alpha_method(&encoded) == 1);
        let oracle = dwebp_rgba(
            &dwebp,
            &scratch.0,
            name,
            &encoded,
            image.width,
            image.height,
        );
        assert_eq!(
            alpha_plane(&oracle),
            alpha_plane(&image.rgba),
            "{name}: dwebp alpha differs"
        );
    }
    assert!(
        lossless_payloads > 0,
        "corpus must exercise emitted headerless-VP8L alpha"
    );
}

#[test]
fn level_reduced_alpha_matches_pinned_dwebp_on_upstream_vectors() {
    let Some(corpus) = corpus_root() else {
        eprintln!("skip ALPH encoder corpus: fetch libwebp-test-data");
        return;
    };
    let Some(dwebp) = pinned_dwebp() else {
        eprintln!("skip ALPH encoder oracle: fetch pinned libwebp");
        return;
    };
    let scratch = ScratchDirectory::new();
    let mut changed_planes = 0_usize;
    for name in ALPHA_VECTORS {
        let source = fs::read(corpus.join(name)).expect("read upstream ALPH vector");
        let image = decode(&source, &DecodeOptions::default()).expect("decode source ALPH vector");
        for quality in [0, 70, 99] {
            let encoded = encode_lossy_rgba_with_alpha_options(
                image.width,
                image.height,
                &image.rgba,
                LossyEncodeOptions { quality: 75 },
                AlphaEncodeOptions {
                    compression: AlphaCompression::Lossless,
                    filter: AlphaFilterSelection::Fast,
                    quality,
                },
            )
            .expect("encode level-reduced ALPH vector");
            let rust = decode(&encoded, &DecodeOptions::default())
                .expect("decode level-reduced ALPH vector");
            let output_name = format!("q{quality}-{name}");
            let oracle = dwebp_rgba(
                &dwebp,
                &scratch.0,
                &output_name,
                &encoded,
                image.width,
                image.height,
            );
            changed_planes += usize::from(alpha_plane(&rust.rgba) != alpha_plane(&image.rgba));
            assert_eq!(
                alpha_plane(&oracle),
                alpha_plane(&rust.rgba),
                "{name} at alpha quality {quality}: dwebp alpha differs"
            );
        }
    }
    assert!(changed_planes > 0, "corpus must exercise level reduction");
}

fn encoded_alpha_method(data: &[u8]) -> u8 {
    let offset = data
        .windows(4)
        .position(|window| window == b"ALPH")
        .expect("encoded WebP has ALPH chunk");
    data[offset + 8] & 0b11
}

fn alpha_plane(rgba: &[u8]) -> Vec<u8> {
    rgba.chunks_exact(4).map(|pixel| pixel[3]).collect()
}

fn corpus_root() -> Option<PathBuf> {
    if let Some(root) = bazel_runfiles_root() {
        return root
            .join("third_party/corpus/libwebp-test-data")
            .is_dir()
            .then(|| root.join("third_party/corpus/libwebp-test-data"));
    }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("third_party/corpus/libwebp-test-data");
    root.is_dir().then_some(root)
}

fn pinned_dwebp() -> Option<PathBuf> {
    if let Some(root) = bazel_runfiles_root() {
        return root
            .join("third_party/oracle/libwebp/build/dwebp")
            .is_file()
            .then(|| root.join("third_party/oracle/libwebp/build/dwebp"));
    }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("third_party/oracle/libwebp");
    let expected = expected_oracle_commit()?;
    let actual = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(&root)
        .output()
        .ok()?;
    (actual.status.success() && String::from_utf8_lossy(&actual.stdout).trim() == expected)
        .then(|| root.join("build/dwebp"))
        .filter(|path| path.is_file())
}

fn bazel_runfiles_root() -> Option<PathBuf> {
    let runfiles = std::env::var_os("TEST_SRCDIR")?;
    let workspace = std::env::var_os("TEST_WORKSPACE").unwrap_or_else(|| "_main".into());
    Some(PathBuf::from(runfiles).join(workspace))
}

fn expected_oracle_commit() -> Option<String> {
    let lock = fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("tools/corpus-lock.toml"),
    )
    .ok()?;
    let mut in_libwebp = false;
    for line in lock.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_libwebp = line == "[libwebp]";
        } else if in_libwebp && line.starts_with("commit = ") {
            return Some(
                line.trim_start_matches("commit = ")
                    .trim_matches('"')
                    .into(),
            );
        }
    }
    None
}

fn dwebp_rgba(
    dwebp: &Path,
    scratch: &Path,
    name: &str,
    encoded: &[u8],
    width: u32,
    height: u32,
) -> Vec<u8> {
    let webp_path = scratch.join(name);
    let pam_path = scratch.join(format!("{name}.pam"));
    fs::write(&webp_path, encoded).expect("write encoded ALPH vector");
    let result = Command::new(dwebp)
        .arg(&webp_path)
        .args(["-pam", "-o"])
        .arg(&pam_path)
        .output()
        .expect("run pinned dwebp");
    assert!(
        result.status.success(),
        "{name}: dwebp rejected encoded ALPH payload: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    pam_rgba(&pam_path, width, height)
}

fn pam_rgba(path: &Path, width: u32, height: u32) -> Vec<u8> {
    let bytes = fs::read(path).expect("read PAM output");
    let marker = b"ENDHDR\n";
    let offset = bytes
        .windows(marker.len())
        .position(|window| window == marker)
        .map(|offset| offset + marker.len())
        .expect("PAM header terminator");
    let pixels = &bytes[offset..];
    assert_eq!(pixels.len(), (width * height * 4) as usize);
    pixels.to_vec()
}

struct ScratchDirectory(PathBuf);

impl ScratchDirectory {
    fn new() -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after Unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "alpha-encoder-oracle-{}-{unique}-{}",
            std::process::id(),
            SCRATCH_SEQUENCE.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir(&path).expect("create ALPH oracle scratch directory");
        Self(path)
    }
}

impl Drop for ScratchDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
