//! Locked-libwebp validation for the first emitted VP8 key-frame slice.

use std::{
    fs,
    path::PathBuf,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

#[test]
fn neutral_vp8_key_frame_is_accepted_by_pinned_dwebp() {
    let Some(dwebp) = pinned_dwebp() else {
        eprintln!("skip VP8 encoder oracle: fetch the pinned libwebp oracle");
        return;
    };
    let scratch = ScratchDirectory::new();
    let payload = webp_vp8::encode_neutral_key_frame(17, 3).expect("emit neutral VP8 payload");
    let mut body = b"WEBP".to_vec();
    body.extend_from_slice(b"VP8 ");
    body.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    body.extend_from_slice(&payload);
    if payload.len() & 1 != 0 {
        body.push(0);
    }
    let mut webp = b"RIFF".to_vec();
    webp.extend_from_slice(&(body.len() as u32).to_le_bytes());
    webp.extend_from_slice(&body);
    let source = scratch.0.join("neutral.webp");
    let target = scratch.0.join("neutral.pam");
    fs::write(&source, webp).expect("write neutral VP8 WebP");
    let output = Command::new(dwebp)
        .arg(&source)
        .args(["-pam", "-o"])
        .arg(&target)
        .output()
        .expect("run pinned dwebp");
    assert!(
        output.status.success(),
        "pinned dwebp rejected neutral VP8 key frame: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let pam = fs::read(target).expect("read neutral VP8 PAM");
    assert!(
        pam.windows(b"WIDTH 17".len())
            .any(|line| line == b"WIDTH 17")
    );
    assert!(
        pam.windows(b"HEIGHT 3".len())
            .any(|line| line == b"HEIGHT 3")
    );
}

#[test]
fn dc_residual_vp8_key_frame_is_accepted_by_pinned_dwebp() {
    let Some(dwebp) = pinned_dwebp() else {
        eprintln!("skip VP8 encoder oracle: fetch the pinned libwebp oracle");
        return;
    };
    let source = webp_vp8::Vp8SourceYuv {
        width: 16,
        height: 16,
        y_stride: 16,
        uv_stride: 8,
        y: vec![134; 16 * 16],
        u: vec![128; 8 * 8],
        v: vec![128; 8 * 8],
    };
    let payload = webp_vp8::encode_dc_predicted_macroblock_key_frame(&source)
        .expect("emit non-neutral VP8 payload");
    let scratch = ScratchDirectory::new();
    let source_path = scratch.0.join("dc-residual.webp");
    let target = scratch.0.join("dc-residual.pam");
    fs::write(&source_path, webp_from_vp8_payload(&payload)).expect("write VP8 WebP");
    let output = Command::new(dwebp)
        .arg(&source_path)
        .args(["-pam", "-o"])
        .arg(&target)
        .output()
        .expect("run pinned dwebp");
    assert!(
        output.status.success(),
        "pinned dwebp rejected DC-residual VP8 key frame: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let pam = fs::read(target).expect("read non-neutral VP8 PAM");
    let pixels = pam
        .split(|&byte| byte == b'\n')
        .scan(false, |past_header, line| {
            if *past_header {
                return None;
            }
            if line == b"ENDHDR" {
                *past_header = true;
            }
            Some(())
        })
        .count();
    assert!(pixels > 0, "oracle produced a PAM header");
}

fn webp_from_vp8_payload(payload: &[u8]) -> Vec<u8> {
    let mut body = b"WEBP".to_vec();
    body.extend_from_slice(b"VP8 ");
    body.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    body.extend_from_slice(payload);
    if payload.len() & 1 != 0 {
        body.push(0);
    }
    let mut webp = b"RIFF".to_vec();
    webp.extend_from_slice(&(body.len() as u32).to_le_bytes());
    webp.extend_from_slice(&body);
    webp
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
            std::env::temp_dir().join(format!("webp-vp8-encoder-{}-{unique}", std::process::id()));
        fs::create_dir_all(&path).expect("create scratch directory");
        Self(path)
    }
}

impl Drop for ScratchDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
