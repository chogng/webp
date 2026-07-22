#![cfg(all(feature = "animation", feature = "encode"))]
//! Optional animation-encoder container oracle tests against locked libwebp.

use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use webp::AnimationEncodeFrame;
use webp::AnimationEncodeOptions;
use webp::encode_lossless_animation;

#[test]
fn lossless_animation_output_is_accepted_by_pinned_webpmux_and_dwebp() {
    let Some((webpmux, dwebp)) = pinned_tools() else {
        eprintln!("skip animation encoder oracle: fetch the pinned libwebp oracle");
        return;
    };
    let scratch = ScratchDirectory::new("animation-encoder-oracle");
    let first = [1, 2, 3, 255, 4, 5, 6, 255];
    let second = [100, 110, 120, 128];
    let encoded = encode_lossless_animation(
        3,
        1,
        &[
            AnimationEncodeFrame {
                x: 0,
                y: 0,
                width: 2,
                height: 1,
                duration_ms: 10,
                rgba: &first,
                dispose_to_background: true,
                blend: false,
            },
            AnimationEncodeFrame {
                x: 2,
                y: 0,
                width: 1,
                height: 1,
                duration_ms: 20,
                rgba: &second,
                dispose_to_background: false,
                blend: false,
            },
        ],
        AnimationEncodeOptions::default(),
    )
    .expect("encode lossless animation");
    let webp_path = scratch.0.join("animation.webp");
    let first_frame_path = scratch.0.join("first-frame.webp");
    let pam_path = scratch.0.join("first-frame.pam");
    fs::write(&webp_path, encoded).expect("write encoded animation");

    let mux_output = Command::new(&webpmux)
        .args(["-info"])
        .arg(&webp_path)
        .output()
        .expect("run pinned webpmux");
    assert!(
        mux_output.status.success(),
        "pinned webpmux rejected lossless animation: {}",
        String::from_utf8_lossy(&mux_output.stderr)
    );
    let extract_output = Command::new(&webpmux)
        .args(["-get", "frame", "1"])
        .arg(&webp_path)
        .arg("-o")
        .arg(&first_frame_path)
        .output()
        .expect("extract first frame through pinned webpmux");
    assert!(
        extract_output.status.success(),
        "pinned webpmux could not extract encoded animation frame: {}",
        String::from_utf8_lossy(&extract_output.stderr)
    );
    let decode_output = Command::new(dwebp)
        .arg(&first_frame_path)
        .arg("-pam")
        .arg("-o")
        .arg(&pam_path)
        .output()
        .expect("run pinned dwebp");
    assert!(
        decode_output.status.success(),
        "pinned dwebp rejected extracted lossless animation frame: {}",
        String::from_utf8_lossy(&decode_output.stderr)
    );
    assert_eq!(pam_rgba(&pam_path), [1, 2, 3, 255, 4, 5, 6, 255]);
}

fn pinned_tools() -> Option<(PathBuf, PathBuf)> {
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
    let locked_commit = lock
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
                    .expect("locked commit has a value")
                    .1
                    .trim()
                    .trim_matches('"')
                    .to_owned()
            })
        })
        .expect("libwebp commit is present in corpus lock");
    let head = Command::new("git")
        .arg("-C")
        .arg(&oracle_root)
        .args(["rev-parse", "HEAD"])
        .output()
        .expect("read pinned oracle Git HEAD");
    assert!(head.status.success(), "read pinned oracle Git HEAD");
    assert_eq!(
        String::from_utf8_lossy(&head.stdout).trim(),
        locked_commit,
        "animation oracle must use the locked libwebp revision"
    );
    let tools = (
        oracle_root.join("build/webpmux"),
        oracle_root.join("build/dwebp"),
    );
    (tools.0.is_file() && tools.1.is_file()).then_some(tools)
}

fn pam_rgba(path: &Path) -> Vec<u8> {
    let pam = fs::read(path).expect("read oracle PAM");
    let marker = b"ENDHDR\n";
    let start = pam
        .windows(marker.len())
        .position(|window| window == marker)
        .map(|offset| offset + marker.len())
        .expect("oracle PAM has an ENDHDR marker");
    pam[start..].to_vec()
}

struct ScratchDirectory(PathBuf);

impl ScratchDirectory {
    fn new(label: &str) -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time is after Unix epoch")
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("webp-{label}-{}-{unique}", std::process::id()));
        fs::create_dir_all(&path).expect("create oracle scratch directory");
        Self(path)
    }
}

impl Drop for ScratchDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
