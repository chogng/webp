#![cfg(feature = "alpha-benchmark-internals")]
//! Full-corpus byte and decoder identity for the private ALPH writer controls.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use webp::fuzzing::{BenchmarkWriterVariant, encode_alpha, set_benchmark_writer_variant};
use webp::{
    AlphaCompression, AlphaEncodeOptions, AlphaFilterSelection, DecodeOptions, LossyEncodeOptions,
    decode, encode_lossy_rgba_with_alpha_options,
};

const QUALITIES: [u8; 4] = [0, 70, 99, 100];
const ALPHA_VECTORS: &[&str] = &[
    "alpha_color_cache.webp",
    "alpha_filter_0_method_0.webp",
    "alpha_filter_0_method_1.webp",
    "alpha_filter_1.webp",
    "alpha_filter_1_method_0.webp",
    "alpha_filter_1_method_1.webp",
    "alpha_filter_2.webp",
    "alpha_filter_2_method_0.webp",
    "alpha_filter_2_method_1.webp",
    "alpha_filter_3.webp",
    "alpha_filter_3_method_0.webp",
    "alpha_filter_3_method_1.webp",
    "alpha_no_compression.webp",
    "big_endian_bug_393.webp",
    "dual_transform.webp",
    "lossless1.webp",
    "lossless2.webp",
    "lossless3.webp",
    "lossless4.webp",
    "lossless_big_random_alpha.webp",
    "lossless_vec_1_0.webp",
    "lossless_vec_1_1.webp",
    "lossless_vec_1_10.webp",
    "lossless_vec_1_11.webp",
    "lossless_vec_1_12.webp",
    "lossless_vec_1_13.webp",
    "lossless_vec_1_14.webp",
    "lossless_vec_1_15.webp",
    "lossless_vec_1_2.webp",
    "lossless_vec_1_3.webp",
    "lossless_vec_1_4.webp",
    "lossless_vec_1_5.webp",
    "lossless_vec_1_6.webp",
    "lossless_vec_1_7.webp",
    "lossless_vec_1_8.webp",
    "lossless_vec_1_9.webp",
    "lossy_alpha1.webp",
    "lossy_alpha2.webp",
    "lossy_alpha3.webp",
    "lossy_alpha4.webp",
    "one_color_no_palette.webp",
];
static SCRATCH_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[test]
fn reference_and_packed_writers_are_identical_on_full_matrix() {
    let (Some(corpus_41), Some(corpus_15), Some(dwebp)) = (
        std::env::var_os("WEBP_ALPHA_IDENTITY_CORPUS_41"),
        std::env::var_os("WEBP_ALPHA_IDENTITY_CORPUS_15"),
        std::env::var_os("DWEBP"),
    ) else {
        eprintln!(
            "skipping ALPH writer identity: set WEBP_ALPHA_IDENTITY_CORPUS_41, \
             WEBP_ALPHA_IDENTITY_CORPUS_15, and DWEBP"
        );
        return;
    };
    let corpus_41 = PathBuf::from(corpus_41);
    let corpus_15 = PathBuf::from(corpus_15);
    let dwebp = PathBuf::from(dwebp);
    let mut inputs = ALPHA_VECTORS
        .iter()
        .map(|name| corpus_41.join(name))
        .collect::<Vec<_>>();
    for kind in ["real", "synthetic"] {
        inputs.extend(
            fs::read_dir(corpus_15.join(kind))
                .expect("read generalization corpus")
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| {
                    path.extension()
                        .is_some_and(|extension| extension == "webp")
                }),
        );
    }
    inputs.sort();
    assert_eq!(inputs.len(), 56, "formal matrix requires 56 inputs");

    let scratch = ScratchDirectory::new();
    let mut rows = 0_usize;
    for path in inputs {
        let source = fs::read(&path).expect("read identity source");
        let image = decode(&source, &DecodeOptions::default()).expect("decode identity source");
        let alpha = alpha_plane(&image.rgba);
        for quality in QUALITIES {
            let options = AlphaEncodeOptions {
                compression: AlphaCompression::Lossless,
                filter: AlphaFilterSelection::Fast,
                quality,
            };

            set_benchmark_writer_variant(BenchmarkWriterVariant::Reference);
            let reference_alpha = encode_alpha(&alpha, image.width, image.height, options)
                .expect("encode reference ALPH");
            set_benchmark_writer_variant(BenchmarkWriterVariant::Packed);
            let packed_alpha = encode_alpha(&alpha, image.width, image.height, options)
                .expect("encode packed ALPH");
            assert_eq!(
                reference_alpha,
                packed_alpha,
                "{} q{quality}: ALPH bytes",
                path.display()
            );

            set_benchmark_writer_variant(BenchmarkWriterVariant::Reference);
            let reference = encode_lossy_rgba_with_alpha_options(
                image.width,
                image.height,
                &image.rgba,
                LossyEncodeOptions { quality: 75 },
                options,
            )
            .expect("encode reference WebP");
            set_benchmark_writer_variant(BenchmarkWriterVariant::Packed);
            let packed = encode_lossy_rgba_with_alpha_options(
                image.width,
                image.height,
                &image.rgba,
                LossyEncodeOptions { quality: 75 },
                options,
            )
            .expect("encode packed WebP");
            assert_eq!(
                reference,
                packed,
                "{} q{quality}: complete WebP bytes",
                path.display()
            );

            let project = decode(&packed, &DecodeOptions::default()).expect("project decode");
            let oracle = dwebp_rgba(&dwebp, &scratch.0, rows, &packed, image.width, image.height);
            assert_eq!(
                alpha_plane(&project.rgba),
                alpha_plane(&oracle),
                "{} q{quality}: project/dwebp alpha",
                path.display()
            );
            if quality == 100 {
                assert_eq!(
                    alpha_plane(&project.rgba),
                    alpha,
                    "{} q100: source alpha",
                    path.display()
                );
            }
            println!(
                "identity\t{}\tq{}\t{}\t{:016x}\tproject=ok\tdwebp=ok",
                path.file_name().unwrap().to_string_lossy(),
                quality,
                packed.len(),
                fnv1a(&packed),
            );
            rows += 1;
        }
    }
    assert_eq!(rows, 224);
}

fn alpha_plane(rgba: &[u8]) -> Vec<u8> {
    rgba.chunks_exact(4).map(|pixel| pixel[3]).collect()
}

fn dwebp_rgba(
    dwebp: &Path,
    scratch: &Path,
    row: usize,
    encoded: &[u8],
    width: u32,
    height: u32,
) -> Vec<u8> {
    let webp = scratch.join(format!("{row}.webp"));
    let pam = scratch.join(format!("{row}.pam"));
    fs::write(&webp, encoded).expect("write identity WebP");
    let output = Command::new(dwebp)
        .arg(&webp)
        .args(["-pam", "-o"])
        .arg(&pam)
        .output()
        .expect("run pinned dwebp");
    assert!(
        output.status.success(),
        "pinned dwebp failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let bytes = fs::read(pam).expect("read identity PAM");
    let marker = b"ENDHDR\n";
    let start = bytes
        .windows(marker.len())
        .position(|window| window == marker)
        .map(|offset| offset + marker.len())
        .expect("PAM header");
    assert_eq!(bytes.len() - start, (width * height * 4) as usize);
    bytes[start..].to_vec()
}

fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for &byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100_0000_01b3);
    }
    hash
}

struct ScratchDirectory(PathBuf);

impl ScratchDirectory {
    fn new() -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "alpha-writer-product-identity-{}-{unique}-{}",
            std::process::id(),
            SCRATCH_SEQUENCE.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir(&path).expect("create identity scratch");
        Self(path)
    }
}

impl Drop for ScratchDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
