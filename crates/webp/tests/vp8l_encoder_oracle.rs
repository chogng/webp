//! Optional VP8L encoder oracle tests against the locked local libwebp build.

use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use webp::{Metadata, encode_lossless_rgba, encode_lossless_rgba_with_metadata};

#[test]
fn literal_vp8l_output_round_trips_through_pinned_libwebp() {
    let Some(dwebp) = pinned_dwebp() else {
        eprintln!("skip VP8L encoder oracle: fetch the pinned libwebp oracle");
        return;
    };

    let scratch = ScratchDirectory::new("vp8l-encoder-oracle");
    for (case, (width, height, rgba)) in opaque_cases().into_iter().enumerate() {
        let actual = decode_with_oracle(&dwebp, &scratch, case, width, height, &rgba);
        assert_eq!(
            actual, rgba,
            "pinned dwebp pixels differ for encoder case {case}"
        );
    }

    for (case, (width, height, rgba)) in alpha_cases().into_iter().enumerate() {
        let actual = decode_with_oracle(&dwebp, &scratch, 100 + case, width, height, &rgba);
        assert_alpha_and_opaque_rgb_match(&actual, &rgba, case);
    }
}

#[test]
fn metadata_muxed_vp8l_output_decodes_through_pinned_libwebp() {
    let Some(dwebp) = pinned_dwebp() else {
        eprintln!("skip metadata mux oracle: fetch the pinned libwebp oracle");
        return;
    };
    let scratch = ScratchDirectory::new("metadata-mux-oracle");
    let rgba = [1, 2, 3, 255, 17, 34, 51, 255];
    let metadata = Metadata {
        iccp: Some(vec![0, 1, 2]),
        exif: Some(vec![3, 4, 5]),
        xmp: Some(b"<xmp/>".to_vec()),
    };
    let encoded = encode_lossless_rgba_with_metadata(2, 1, &rgba, &metadata)
        .expect("encode metadata VP8L test case");
    let webp_path = scratch.0.join("metadata.webp");
    let pam_path = scratch.0.join("metadata.pam");
    fs::write(&webp_path, encoded).expect("write encoded metadata VP8L test case");
    let output = Command::new(&dwebp)
        .arg(&webp_path)
        .arg("-pam")
        .arg("-o")
        .arg(&pam_path)
        .output()
        .expect("run pinned dwebp");
    assert!(
        output.status.success(),
        "pinned dwebp rejected metadata-muxed VP8L: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(pam_rgba(&pam_path, 2, 1), rgba);
}

fn opaque_cases() -> Vec<(u32, u32, Vec<u8>)> {
    let mut cases = vec![
        (1, 1, vec![0, 0, 0, 255]),
        (3, 1, vec![1, 2, 3, 255, 17, 34, 51, 255, 255, 127, 63, 255]),
        (
            5,
            1,
            vec![
                0, 255, 1, 255, 13, 8, 21, 255, 34, 55, 89, 255, 144, 233, 5, 255, 255, 0, 127, 255,
            ],
        ),
    ];
    let mut block_edge_rgba = Vec::new();
    for index in 0..25_u8 {
        block_edge_rgba.extend_from_slice(&[
            index.wrapping_mul(17),
            index.wrapping_mul(31),
            index.wrapping_mul(47),
            255,
        ]);
    }
    cases.push((5, 5, block_edge_rgba));
    cases
}

fn alpha_cases() -> Vec<(u32, u32, Vec<u8>)> {
    let mut cases = vec![
        (1, 1, vec![200, 100, 50, 0]),
        (3, 1, vec![1, 2, 3, 4, 240, 120, 60, 128, 17, 34, 51, 255]),
    ];
    let mut block_edge_rgba = Vec::new();
    for index in 0..25_u8 {
        block_edge_rgba.extend_from_slice(&[
            index.wrapping_mul(17),
            index.wrapping_mul(31),
            index.wrapping_mul(47),
            index.wrapping_mul(61),
        ]);
    }
    cases.push((5, 5, block_edge_rgba));
    cases
}

fn decode_with_oracle(
    dwebp: &Path,
    scratch: &ScratchDirectory,
    case: usize,
    width: u32,
    height: u32,
    rgba: &[u8],
) -> Vec<u8> {
    let encoded = encode_lossless_rgba(width, height, rgba).expect("encode VP8L test case");
    let webp_path = scratch.0.join(format!("case-{case}.webp"));
    let pam_path = scratch.0.join(format!("case-{case}.pam"));
    fs::write(&webp_path, encoded).expect("write encoded VP8L test case");

    let output = Command::new(dwebp)
        .arg(&webp_path)
        .arg("-pam")
        .arg("-o")
        .arg(&pam_path)
        .output()
        .expect("run pinned dwebp");
    assert!(
        output.status.success(),
        "pinned dwebp rejected encoder case {case}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    pam_rgba(&pam_path, width, height)
}

fn assert_alpha_and_opaque_rgb_match(actual: &[u8], expected: &[u8], case: usize) {
    for (pixel, expected_pixel) in actual.chunks_exact(4).zip(expected.chunks_exact(4)) {
        // dwebp's PAM writer premultiplies translucent RGB. The straight-alpha
        // channel remains comparable for every pixel, while RGB is canonical
        // only for opaque samples.
        assert_eq!(pixel[3], expected_pixel[3], "alpha differs for case {case}");
        if pixel[3] == 255 {
            assert_eq!(
                pixel[..3],
                expected_pixel[..3],
                "opaque RGB differs for case {case}"
            );
        }
    }
}

fn pinned_dwebp() -> Option<PathBuf> {
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
        .expect("read local oracle revision");
    assert!(head.status.success(), "read pinned oracle Git HEAD");
    assert_eq!(
        String::from_utf8_lossy(&head.stdout).trim(),
        locked_commit,
        "encoder oracle must use the locked libwebp revision"
    );

    let dwebp = oracle_root.join("build/dwebp");
    dwebp.is_file().then_some(dwebp)
}

fn pam_rgba(path: &Path, width: u32, height: u32) -> Vec<u8> {
    let pam = fs::read(path).expect("read oracle RGBA PAM");
    let marker = b"ENDHDR\n";
    let start = pam
        .windows(marker.len())
        .position(|window| window == marker)
        .map(|offset| offset + marker.len())
        .expect("oracle PAM has an ENDHDR marker");
    let rgba = pam[start..].to_vec();
    assert_eq!(
        rgba.len(),
        usize::try_from(width * height * 4).expect("small oracle test dimensions"),
        "oracle PAM has expected RGBA byte count"
    );
    rgba
}

struct ScratchDirectory(PathBuf);

impl ScratchDirectory {
    fn new(prefix: &str) -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock is after Unix epoch")
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("webp-{prefix}-{}-{unique}", std::process::id()));
        fs::create_dir(&path).expect("create isolated oracle scratch directory");
        Self(path)
    }
}

impl Drop for ScratchDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
