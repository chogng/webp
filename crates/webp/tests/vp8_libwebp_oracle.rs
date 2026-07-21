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

#[test]
#[ignore = "pending VP8 lossy pixel conformance"]
fn lossy_vp8_sample_matches_libwebp_yuv() {
    let Some((input, dwebp)) = local_oracle() else {
        eprintln!("skip VP8 pixel oracle: set LIBWEBP_ORACLE_ROOT or install dwebp");
        return;
    };
    let scratch = ScratchDirectory::new("vp8-libwebp-yuv-oracle");
    let reference = scratch.0.join("reference.yuv");
    let output = Command::new(dwebp)
        .arg(&input)
        .args(["-yuv", "-o"])
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
    let options = DecodeOptions::default();
    let container = webp_container::parse(&input_bytes, options.compatibility, &options.limits)
        .expect("parse VP8 WebP container");
    let payload = container
        .chunks()
        .iter()
        .find(|chunk| chunk.fourcc == webp_container::VP8)
        .expect("VP8 chunk")
        .payload;
    let header =
        webp_vp8::parse_riff_payload(payload, None, &options.limits).expect("parse VP8 payload");
    let actual =
        webp_vp8::decode_intra_frame(payload, &header, &options.limits).expect("decode VP8 YUV");
    let expected = fs::read(&reference)
        .unwrap_or_else(|error| panic!("read {}: {error}", reference.display()));
    assert_yuv_matches_libwebp(&actual, &expected);
}

fn assert_yuv_matches_libwebp(actual: &webp_vp8::Vp8YuvImage, expected: &[u8]) {
    let width = usize::try_from(actual.width).expect("width fits usize");
    let height = usize::try_from(actual.height).expect("height fits usize");
    let uv_width = width.div_ceil(2);
    let uv_height = height.div_ceil(2);
    let y_len = width * height;
    let uv_len = uv_width * uv_height;
    assert_eq!(expected.len(), y_len + 2 * uv_len, "YUV byte length");
    assert_plane_matches_libwebp(
        "Y",
        &actual.y,
        actual.y_stride,
        width,
        height,
        &expected[..y_len],
    );
    assert_plane_matches_libwebp(
        "U",
        &actual.u,
        actual.uv_stride,
        uv_width,
        uv_height,
        &expected[y_len..y_len + uv_len],
    );
    assert_plane_matches_libwebp(
        "V",
        &actual.v,
        actual.uv_stride,
        uv_width,
        uv_height,
        &expected[y_len + uv_len..],
    );
}

fn assert_plane_matches_libwebp(
    name: &str,
    actual: &[u8],
    stride: usize,
    width: usize,
    height: usize,
    expected: &[u8],
) {
    let actual = actual
        .chunks_exact(stride)
        .take(height)
        .flat_map(|row| row[..width].iter().copied())
        .collect::<Vec<_>>();
    let mismatches = actual
        .iter()
        .zip(expected)
        .filter(|(actual, expected)| actual != expected)
        .count();
    let Some((index, (actual, expected))) = actual
        .iter()
        .zip(expected)
        .enumerate()
        .find(|(_, (actual, expected))| actual != expected)
    else {
        return;
    };
    panic!(
        "VP8 {name} differs from libwebp at ({}, {}): actual {actual}, expected {expected}; {mismatches} mismatched samples",
        index % width,
        index / width,
    );
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
