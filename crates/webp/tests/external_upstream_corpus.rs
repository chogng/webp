use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use webp::{DecodeOptions, decode};
use webp_container::VP8L;
use webp_testkit::{Codec, FixtureApi, FixtureClass, FixtureRunner, sha256_hex};

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

#[test]
fn selected_upstream_lossless_vectors_decode_to_recorded_rgba_goldens() {
    let Some(root) = corpus_root() else {
        return;
    };

    let summary = FixtureRunner::with_fixture_root(root.join("manifests"), &root)
        .run_all(|fixture, bytes| {
            if fixture.class != FixtureClass::MustAccept {
                return Ok::<_, String>(());
            }
            if fixture.codec != Codec::Vp8l || fixture.api != FixtureApi::Decode {
                return Err(format!("{} is not an M1 VP8L decode fixture", fixture.id));
            }
            let image = decode(bytes, &DecodeOptions::default())
                .map_err(|error| format!("{}: decode failed: {error}", fixture.id))?;
            let width = fixture
                .expected_width
                .expect("validated accepted fixture width");
            let height = fixture
                .expected_height
                .expect("validated accepted fixture height");
            let rgba = fixture
                .expected_rgba_sha256
                .as_deref()
                .expect("validated M1 fixture RGBA golden");
            if (image.width, image.height) != (width, height) {
                return Err(format!(
                    "{}: dimensions {}x{} != {width}x{height}",
                    fixture.id, image.width, image.height
                ));
            }
            if sha256_hex(&image.rgba) != rgba {
                return Err(format!("{}: RGBA hash mismatch", fixture.id));
            }
            Ok(())
        })
        .expect("all selected upstream vectors must satisfy their manifest contract");

    assert_eq!(summary.fixtures, 68, "selected upstream corpus size");
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

        let container = webp_container::parse(&bytes, options.compatibility, &options.limits)
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
