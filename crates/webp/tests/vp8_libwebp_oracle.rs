//! Optional pixel-level VP8 differential test against a local libwebp build.

use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use webp::{DecodeOptions, decode};

#[test]
#[ignore = "pending VP8 lossy pixel conformance"]
fn lossy_vp8_sample_matches_libwebp_rgba() {
    let Some((input, dwebp)) = local_oracle() else {
        eprintln!("skip VP8 pixel oracle: set LIBWEBP_ORACLE_ROOT or install dwebp");
        return;
    };
    let scratch = ScratchDirectory::new("vp8-libwebp-oracle");
    let reference = scratch.0.join("reference.pam");
    let output = Command::new(dwebp)
        .arg(&input)
        .args(["-pam", "-o"])
        .arg(&reference)
        .output()
        .expect("run libwebp dwebp");
    assert!(
        output.status.success(),
        "libwebp dwebp failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let input_bytes = fs::read(&input)
        .unwrap_or_else(|error| panic!("read VP8 oracle sample {}: {error}", input.display()));
    let decoded = decode(&input_bytes, &DecodeOptions::default())
        .unwrap_or_else(|error| panic!("our VP8 decoder rejected {}: {error}", input.display()));
    let (width, height, rgba) = pam_rgba(&reference);
    assert_eq!((decoded.width, decoded.height), (width, height));
    assert_rgba_matches_libwebp(&decoded.rgba, &rgba, width);
}

fn assert_rgba_matches_libwebp(actual: &[u8], expected: &[u8], width: u32) {
    assert_eq!(actual.len(), expected.len(), "RGBA byte length");
    let mismatched_pixels = actual
        .chunks_exact(4)
        .zip(expected.chunks_exact(4))
        .filter(|(actual, expected)| actual != expected)
        .count();
    let Some((pixel, (actual, expected))) = actual
        .chunks_exact(4)
        .zip(expected.chunks_exact(4))
        .enumerate()
        .find(|(_, (actual, expected))| actual != expected)
    else {
        return;
    };
    panic!(
        "VP8 RGBA differs from libwebp at ({}, {}): actual {:?}, expected {:?}; {mismatched_pixels} mismatched pixels",
        pixel % usize::try_from(width).expect("width fits usize"),
        pixel / usize::try_from(width).expect("width fits usize"),
        actual,
        expected,
    );
}

fn local_oracle() -> Option<(PathBuf, PathBuf)> {
    let root = std::env::var_os("LIBWEBP_ORACLE_ROOT")
        .map(PathBuf::from)
        .or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../libwebp")
                .is_dir()
                .then(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../libwebp"))
        })?;
    let input = root.join("examples/test.webp");
    if !input.is_file() {
        return None;
    }
    let dwebp = std::env::var_os("DWEBP")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("dwebp"));
    Command::new(&dwebp)
        .arg("-version")
        .output()
        .ok()
        .filter(|output| output.status.success())?;
    Some((input, dwebp))
}

fn pam_rgba(path: &Path) -> (u32, u32, Vec<u8>) {
    let bytes = fs::read(path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    let header_end = bytes
        .windows(b"ENDHDR\n".len())
        .position(|window| window == b"ENDHDR\n")
        .map(|index| index + b"ENDHDR\n".len())
        .expect("PAM ENDHDR");
    let header = std::str::from_utf8(&bytes[..header_end]).expect("ASCII PAM header");
    let width = pam_header_value(header, "WIDTH");
    let height = pam_header_value(header, "HEIGHT");
    assert_eq!(pam_header_value(header, "DEPTH"), 4, "RGBA PAM depth");
    let length = usize::try_from(width)
        .and_then(|width| usize::try_from(height).map(|height| width * height * 4))
        .expect("PAM dimensions fit usize");
    assert_eq!(bytes.len() - header_end, length, "PAM pixel length");
    (width, height, bytes[header_end..].to_vec())
}

fn pam_header_value(header: &str, key: &str) -> u32 {
    header
        .lines()
        .find_map(|line| {
            line.strip_prefix(key)
                .and_then(|value| value.trim().parse().ok())
        })
        .unwrap_or_else(|| panic!("PAM header contains {key}"))
}

struct ScratchDirectory(PathBuf);

impl ScratchDirectory {
    fn new(prefix: &str) -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock is after Unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("{prefix}-{}-{unique}", std::process::id()));
        fs::create_dir(&path).expect("create VP8 oracle scratch directory");
        Self(path)
    }
}

impl Drop for ScratchDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
