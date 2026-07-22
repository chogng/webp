#![cfg(feature = "decode")]

use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use webp_container::VP8L;
use webp_decode::DecodeOptions;
use webp_decode::decode;

const UPSTREAM_SMOKE_SELECTION: &str =
    include_str!("../../../tests/corpora/libwebp-test-data-smoke-v1.txt");
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

fn corpus_root() -> Option<PathBuf> {
    let cargo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("third_party/corpus/libwebp-test-data");
    match std::env::var_os("TEST_SRCDIR") {
        Some(runfiles) => {
            let workspace = std::env::var_os("TEST_WORKSPACE").unwrap_or_else(|| "_main".into());
            let root = PathBuf::from(runfiles)
                .join(workspace)
                .join("third_party/corpus/libwebp-test-data");
            assert!(
                root.is_dir(),
                "Bazel external-corpus test requires the fetched libwebp corpus"
            );
            Some(root)
        }
        None if cargo_root.is_dir() => Some(cargo_root),
        None => None,
    }
}

fn selected_lossless_vectors() -> impl Iterator<Item = &'static str> {
    UPSTREAM_SMOKE_SELECTION
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter(|name| {
            matches!(
                *name,
                "lossless1.webp"
                    | "lossless2.webp"
                    | "lossless3.webp"
                    | "lossless4.webp"
                    | "lossless_big_random_alpha.webp"
                    | "lossless_color_transform.webp"
                    | "color_cache_bits_11.webp"
                    | "dual_transform.webp"
                    | "one_color_no_palette.webp"
            ) || name.starts_with("lossless_vec_1_")
                || name.starts_with("lossless_vec_2_")
        })
}

#[test]
fn selected_upstream_lossless_vectors_decode_directly() {
    let Some(root) = corpus_root() else {
        return;
    };

    let names = selected_lossless_vectors().collect::<Vec<_>>();
    assert_eq!(names.len(), 41, "selected lossless corpus size");
    for name in names {
        let path = root.join(name);
        let bytes =
            fs::read(&path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
        let image = decode(&bytes, &DecodeOptions::default())
            .unwrap_or_else(|error| panic!("{name}: decode failed: {error}"));
        assert!(
            image.width > 0 && image.height > 0,
            "{name}: image dimensions"
        );
        assert_eq!(
            image.rgba.len(),
            usize::try_from(image.width * image.height * 4).expect("small fixture dimensions"),
            "{name}: RGBA length"
        );
    }
}

#[test]
fn selected_upstream_alpha_vectors_decode_with_non_opaque_alpha() {
    let Some(root) = corpus_root() else {
        return;
    };
    let oracle = pinned_oracle_root().map(|root| root.join("build/dwebp"));
    let oracle = oracle.filter(|dwebp| dwebp.is_file());
    let scratch = oracle.as_ref().map(|_| {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock is after Unix epoch")
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("alpha-oracle-{}-{unique}", std::process::id()));
        fs::create_dir(&path).expect("create alpha-oracle scratch directory");
        ScratchDirectory(path)
    });
    for name in ALPHA_VECTORS {
        let path = root.join(name);
        let bytes =
            fs::read(&path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
        let image = decode(&bytes, &DecodeOptions::default())
            .unwrap_or_else(|error| panic!("{name}: alpha decode failed: {error}"));
        assert!(
            image.rgba.chunks_exact(4).any(|pixel| pixel[3] != 255),
            "{name}: fixture must exercise non-opaque alpha"
        );
        if let (Some(dwebp), Some(scratch)) = (&oracle, &scratch) {
            let output = scratch.0.join(format!("{name}.pam"));
            let result = Command::new(dwebp)
                .arg(&path)
                .arg("-pam")
                .arg("-o")
                .arg(&output)
                .output()
                .expect("run pinned dwebp for alpha vector");
            assert!(
                result.status.success(),
                "{name}: dwebp failed: {}",
                String::from_utf8_lossy(&result.stderr)
            );
            let oracle = pam_rgba(&output, image.width, image.height);
            for (actual, expected) in image.rgba.chunks_exact(4).zip(oracle.chunks_exact(4)) {
                // dwebp's PAM writer premultiplies RGB for translucent pixels,
                // whereas this crate's public contract is straight RGBA. Alpha
                // remains directly comparable, as do RGB components at alpha 255.
                assert_eq!(
                    actual[3], expected[3],
                    "{name}: alpha differs from pinned dwebp"
                );
                if actual[3] == 255 {
                    assert_eq!(actual[..3], expected[..3], "{name}: opaque RGB differs");
                }
            }
        }
    }
}

#[test]
fn small_upstream_lossless_vectors_do_not_panic_at_any_byte_truncation() {
    let Some(root) = corpus_root() else {
        return;
    };
    let options = DecodeOptions::default();

    for transform_mask in 0..16 {
        let name = format!("lossless_vec_1_{transform_mask}.webp");
        let bytes = std::fs::read(root.join(&name)).expect("read selected small VP8L fixture");

        for cut in 0..bytes.len() {
            let outcome = std::panic::catch_unwind(|| decode(&bytes[..cut], &options));
            assert!(
                outcome.is_ok(),
                "{name}: public decoder panicked at file byte {cut}"
            );
        }

        let container = webp_demux::parse(
            &bytes,
            webp_demux::CompatibilityProfile::SpecStrict,
            &webp_demux::ContainerLimits::default(),
        )
        .expect("selected fixture has a valid strict RIFF container");
        let payload = container
            .chunks()
            .iter()
            .find(|chunk| chunk.fourcc == VP8L)
            .expect("selected lossless fixture has a VP8L chunk")
            .payload;

        // Rebuild a valid RIFF envelope around each payload prefix. This gets
        // past container framing so every VP8L byte boundary reaches the real
        // lossless decoder rather than stopping at an incomplete chunk size.
        for cut in 0..payload.len() {
            let truncated = wrap_vp8l(&payload[..cut]);
            let outcome = std::panic::catch_unwind(|| decode(&truncated, &options));
            assert!(
                outcome.is_ok(),
                "{name}: VP8L decoder panicked at payload byte {cut}"
            );
        }
    }
}

#[test]
fn deterministic_small_rgba_encoded_by_pinned_libwebp_round_trips_in_both_decoders() {
    let Some(oracle_root) = pinned_oracle_root() else {
        return;
    };
    let cwebp = oracle_root.join("build/cwebp");
    let dwebp = oracle_root.join("build/dwebp");
    if !cwebp.is_file() || !dwebp.is_file() {
        return;
    }

    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock is after the Unix epoch")
        .as_nanos();
    let scratch_path = std::env::temp_dir().join(format!(
        "webp-m1-random-roundtrip-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir(&scratch_path).expect("create isolated random-roundtrip directory");
    let scratch = ScratchDirectory(scratch_path);

    let mut random_state = 0x6d_31_5a_c7_u32;
    for case in 0..32_u32 {
        let (width, height) = match case {
            0 => (1, 1),
            1 => (1, 16),
            2 => (16, 1),
            3 => (3, 5),
            _ => (
                next_random(&mut random_state) % 16 + 1,
                next_random(&mut random_state) % 16 + 1,
            ),
        };
        let pixel_bytes = usize::try_from(width * height * 4).expect("small image fits usize");
        let mut rgba = Vec::with_capacity(pixel_bytes);
        for _ in 0..pixel_bytes {
            rgba.push(next_random(&mut random_state) as u8);
        }

        let input = scratch.0.join(format!("case-{case:02}.pam"));
        let encoded = scratch.0.join(format!("case-{case:02}.webp"));
        let oracle_output = scratch.0.join(format!("case-{case:02}-decoded.pam"));
        write_rgba_pam(&input, width, height, &rgba);

        let cwebp_output = Command::new(&cwebp)
            .args(["-quiet", "-lossless", "-exact", "-m"])
            .arg((case % 7).to_string())
            .arg(&input)
            .arg("-o")
            .arg(&encoded)
            .output()
            .expect("run pinned cwebp");
        assert!(
            cwebp_output.status.success(),
            "cwebp failed for case {case}: {}",
            String::from_utf8_lossy(&cwebp_output.stderr)
        );

        let webp_bytes = fs::read(&encoded).expect("read cwebp output");
        let decoded = decode(&webp_bytes, &DecodeOptions::default())
            .unwrap_or_else(|error| panic!("our decoder rejected random case {case}: {error}"));
        assert_eq!((decoded.width, decoded.height), (width, height));
        assert_eq!(
            decoded.rgba, rgba,
            "our decoder RGBA mismatch for case {case}"
        );

        let dwebp_output = Command::new(&dwebp)
            .arg(&encoded)
            .arg("-pam")
            .arg("-o")
            .arg(&oracle_output)
            .output()
            .expect("run pinned dwebp");
        assert!(
            dwebp_output.status.success(),
            "dwebp failed for case {case}: {}",
            String::from_utf8_lossy(&dwebp_output.stderr)
        );
        assert_eq!(
            pam_rgba(&oracle_output, width, height),
            rgba,
            "pinned dwebp RGBA mismatch for case {case}"
        );
    }
}

struct ScratchDirectory(PathBuf);

impl Drop for ScratchDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn pinned_oracle_root() -> Option<PathBuf> {
    if std::env::var_os("TEST_SRCDIR").is_some() {
        return None;
    }
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let oracle_root = repository.join("third_party/oracle/libwebp");
    if !oracle_root.join(".git").is_dir() {
        return None;
    }

    let lock = fs::read_to_string(repository.join("tools/corpus-lock.toml"))
        .expect("read corpus lock for pinned oracle revision");
    let mut in_libwebp = false;
    let mut locked_commit = None;
    for line in lock.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_libwebp = line == "[libwebp]";
        } else if in_libwebp && line.starts_with("commit =") {
            locked_commit = line
                .split_once('=')
                .map(|(_, value)| value.trim().trim_matches('"').to_owned());
            break;
        }
    }
    let locked_commit = locked_commit.expect("libwebp commit is present in corpus lock");
    let head = Command::new("git")
        .arg("-C")
        .arg(&oracle_root)
        .args(["rev-parse", "HEAD"])
        .output()
        .expect("read local oracle revision");
    assert!(head.status.success(), "read pinned oracle Git HEAD");
    assert_eq!(
        String::from_utf8_lossy(&head.stdout).trim(),
        locked_commit,
        "random roundtrip must use the locked libwebp oracle"
    );
    Some(oracle_root)
}

fn next_random(state: &mut u32) -> u32 {
    *state ^= *state << 13;
    *state ^= *state >> 17;
    *state ^= *state << 5;
    *state
}

fn write_rgba_pam(path: &Path, width: u32, height: u32, rgba: &[u8]) {
    let mut pam = format!(
        "P7\nWIDTH {width}\nHEIGHT {height}\nDEPTH 4\nMAXVAL 255\nTUPLTYPE RGB_ALPHA\nENDHDR\n"
    )
    .into_bytes();
    pam.extend_from_slice(rgba);
    fs::write(path, pam).expect("write deterministic RGBA PAM");
}

fn pam_rgba(path: &Path, width: u32, height: u32) -> Vec<u8> {
    let pam = fs::read(path).expect("read oracle RGBA PAM");
    let marker = b"ENDHDR\n";
    let pixel_start = pam
        .windows(marker.len())
        .position(|window| window == marker)
        .map(|position| position + marker.len())
        .expect("oracle PAM has an ENDHDR marker");
    let rgba = pam[pixel_start..].to_vec();
    assert_eq!(
        rgba.len(),
        usize::try_from(width * height * 4).expect("small image fits usize"),
        "oracle PAM has the expected RGBA byte count"
    );
    rgba
}

fn wrap_vp8l(payload: &[u8]) -> Vec<u8> {
    let padding = payload.len() & 1;
    let riff_size = 12 + payload.len() + padding;
    let mut file = Vec::with_capacity(20 + payload.len() + padding);
    file.extend_from_slice(b"RIFF");
    file.extend_from_slice(
        &u32::try_from(riff_size)
            .expect("small truncation fixture fits RIFF length")
            .to_le_bytes(),
    );
    file.extend_from_slice(b"WEBPVP8L");
    file.extend_from_slice(
        &u32::try_from(payload.len())
            .expect("small truncation fixture fits VP8L chunk length")
            .to_le_bytes(),
    );
    file.extend_from_slice(payload);
    if padding != 0 {
        file.push(0);
    }
    file
}
