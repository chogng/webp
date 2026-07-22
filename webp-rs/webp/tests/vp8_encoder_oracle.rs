#![cfg(feature = "encode")]
//! Locked-libwebp validation for the first emitted VP8 key-frame slice.

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use webp::DecodeOptions;
use webp::LossyEncodeOptions;
use webp::decode;
use webp::encode_lossy_rgba_with_options;

#[test]
fn public_lossy_vp8_profile_matches_pinned_dwebp_pixels() {
    let Some(dwebp) = pinned_dwebp() else {
        eprintln!("skip VP8 encoder oracle: fetch the pinned libwebp oracle");
        return;
    };
    let mut rgba = Vec::new();
    for y in 0_u8..16 {
        for x in 0_u8..16 {
            rgba.extend_from_slice(&[
                x.wrapping_mul(15),
                y.wrapping_mul(15),
                x.wrapping_add(y).wrapping_mul(7),
                255,
            ]);
        }
    }
    let scratch = ScratchDirectory::new();
    for quality in [0, 75, 100] {
        let encoded = encode_lossy_rgba_with_options(16, 16, &rgba, LossyEncodeOptions { quality })
            .expect("encode public lossy VP8 profile");
        let rust =
            decode(&encoded, &DecodeOptions::default()).expect("decode public lossy VP8 profile");
        let source = scratch.0.join(format!("public-lossy-{quality}.webp"));
        let target = scratch.0.join(format!("public-lossy-{quality}.pam"));
        fs::write(&source, encoded).expect("write public lossy WebP");
        let output = Command::new(&dwebp)
            .arg(&source)
            .args(["-pam", "-o"])
            .arg(&target)
            .output()
            .expect("run pinned dwebp");
        assert!(
            output.status.success(),
            "pinned dwebp rejected public lossy VP8 at quality {quality}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(pam_rgba(&target, 16, 16), rust.rgba, "quality {quality}");
    }
}

#[test]
fn public_lossy_vp8_multimacroblock_profile_matches_pinned_dwebp_pixels() {
    let Some(dwebp) = pinned_dwebp() else {
        eprintln!("skip VP8 encoder oracle: fetch the pinned libwebp oracle");
        return;
    };
    let mut rgba = Vec::new();
    for y in 0_u8..16 {
        for x in 0_u8..32 {
            let [red, green, blue] = if x < 16 {
                [40, y.wrapping_mul(11), 220]
            } else {
                [220, x.wrapping_mul(7), 40]
            };
            rgba.extend_from_slice(&[red, green, blue, 255]);
        }
    }
    let encoded = encode_lossy_rgba_with_options(32, 16, &rgba, LossyEncodeOptions { quality: 75 })
        .expect("encode multi-macroblock public lossy VP8 profile");
    let rust = decode(&encoded, &DecodeOptions::default())
        .expect("decode multi-macroblock public lossy VP8 profile");
    let scratch = ScratchDirectory::new();
    let source = scratch.0.join("public-lossy-multi.webp");
    let target = scratch.0.join("public-lossy-multi.pam");
    fs::write(&source, encoded).expect("write multi-macroblock public lossy WebP");
    let output = Command::new(dwebp)
        .arg(&source)
        .args(["-pam", "-o"])
        .arg(&target)
        .output()
        .expect("run pinned dwebp");
    assert!(
        output.status.success(),
        "pinned dwebp rejected multi-macroblock public lossy VP8: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(pam_rgba(&target, 32, 16), rust.rgba);
}

#[test]
fn public_lossy_vp8_alpha_profile_preserves_oracle_alpha() {
    let Some(dwebp) = pinned_dwebp() else {
        eprintln!("skip VP8 encoder oracle: fetch the pinned libwebp oracle");
        return;
    };
    let mut rgba = Vec::new();
    for y in 0_u8..16 {
        for x in 0_u8..16 {
            rgba.extend_from_slice(&[
                x.wrapping_mul(15),
                y.wrapping_mul(15),
                x.wrapping_add(y).wrapping_mul(7),
                x.wrapping_add(y.wrapping_mul(16)),
            ]);
        }
    }
    let encoded = encode_lossy_rgba_with_options(16, 16, &rgba, LossyEncodeOptions { quality: 75 })
        .expect("encode public lossy VP8 alpha profile");
    let rust =
        decode(&encoded, &DecodeOptions::default()).expect("decode public lossy VP8 alpha profile");
    let scratch = ScratchDirectory::new();
    let source = scratch.0.join("public-lossy-alpha.webp");
    let target = scratch.0.join("public-lossy-alpha.pam");
    fs::write(&source, encoded).expect("write public lossy alpha WebP");
    let output = Command::new(dwebp)
        .arg(&source)
        .args(["-pam", "-o"])
        .arg(&target)
        .output()
        .expect("run pinned dwebp");
    assert!(
        output.status.success(),
        "pinned dwebp rejected public lossy VP8 alpha: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let oracle = pam_rgba(&target, 16, 16);
    assert_eq!(
        oracle
            .chunks_exact(4)
            .map(|pixel| pixel[3])
            .collect::<Vec<_>>(),
        rust.rgba
            .chunks_exact(4)
            .map(|pixel| pixel[3])
            .collect::<Vec<_>>()
    );
    assert_eq!(
        rust.rgba
            .chunks_exact(4)
            .map(|pixel| pixel[3])
            .collect::<Vec<_>>(),
        rgba.chunks_exact(4)
            .map(|pixel| pixel[3])
            .collect::<Vec<_>>()
    );
}

fn pam_rgba(path: &std::path::Path, width: u32, height: u32) -> Vec<u8> {
    let pam = fs::read(path).expect("read oracle PAM");
    let marker = b"ENDHDR\n";
    let start = pam
        .windows(marker.len())
        .position(|window| window == marker)
        .map(|offset| offset + marker.len())
        .expect("oracle PAM has an ENDHDR marker");
    let rgba = pam[start..].to_vec();
    assert_eq!(
        rgba.len(),
        usize::try_from(width * height * 4).expect("small oracle dimensions"),
        "oracle PAM has expected RGBA byte count"
    );
    rgba
}

fn pinned_dwebp() -> Option<PathBuf> {
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let oracle = repository.join("third_party/oracle/libwebp");
    if !oracle.join(".git").is_dir() {
        return None;
    }
    let lock = fs::read_to_string(repository.join("tools/corpus-lock.toml")).expect("read lock");
    let expected = lock
        .lines()
        .map(str::trim)
        .scan(false, |in_libwebp, line| {
            if line.starts_with('[') {
                *in_libwebp = line == "[libwebp]";
            }
            Some((*in_libwebp, line))
        })
        .find_map(|(in_libwebp, line)| {
            (in_libwebp && line.starts_with("commit =")).then(|| {
                line.split_once('=')
                    .expect("lock value")
                    .1
                    .trim()
                    .trim_matches('"')
                    .to_owned()
            })
        })
        .expect("libwebp commit");
    let head = Command::new("git")
        .args([
            "-C",
            oracle.to_str().expect("UTF-8 oracle path"),
            "rev-parse",
            "HEAD",
        ])
        .output()
        .expect("read oracle Git HEAD");
    assert!(head.status.success());
    assert_eq!(String::from_utf8_lossy(&head.stdout).trim(), expected);
    let dwebp = oracle.join("build/dwebp");
    dwebp.is_file().then_some(dwebp)
}

struct ScratchDirectory(PathBuf);

impl ScratchDirectory {
    fn new() -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("vp8-encoder-{}-{unique}", std::process::id()));
        fs::create_dir_all(&path).expect("create scratch directory");
        Self(path)
    }
}

impl Drop for ScratchDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
