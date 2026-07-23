//! End-to-end static VP8L encoder benchmark for a fixed set of WebP files.
//!
//! Usage: `cargo run --release -p webp --example encode_bench -- <iterations> <files...>`

use std::env;
use std::fs;
use std::hint::black_box;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

use webp::DecodeOptions;
use webp::LosslessEncodeOptions;
use webp::LosslessEncodeProfile;
use webp::decode;
use webp::encode_lossless_rgba_with_options;

fn main() -> ExitCode {
    let mut arguments = env::args_os().skip(1);
    let Some(iterations) = arguments.next() else {
        eprintln!("usage: encode_bench <iterations> <files...>");
        return ExitCode::FAILURE;
    };
    let Ok(iterations) = iterations.to_string_lossy().parse::<usize>() else {
        eprintln!("iterations must be a positive integer");
        return ExitCode::FAILURE;
    };
    if iterations == 0 {
        eprintln!("iterations must be greater than zero");
        return ExitCode::FAILURE;
    }
    let paths = arguments.map(PathBuf::from).collect::<Vec<_>>();
    if paths.is_empty() {
        eprintln!("provide at least one WebP file");
        return ExitCode::FAILURE;
    }
    let profile = match env::var("WEBP_RS_LOSSLESS_PROFILE").as_deref() {
        Ok("high-compression") => LosslessEncodeProfile::HighCompression,
        Ok("default") | Err(_) => LosslessEncodeProfile::Default,
        Ok(value) => {
            eprintln!("unsupported WEBP_RS_LOSSLESS_PROFILE={value}");
            return ExitCode::FAILURE;
        }
    };
    let mut options = LosslessEncodeOptions::default();
    options.profile = profile;

    let mut inputs = Vec::with_capacity(paths.len());
    for path in &paths {
        let bytes = match fs::read(path) {
            Ok(bytes) => bytes,
            Err(error) => {
                eprintln!("{}: {error}", path.display());
                return ExitCode::FAILURE;
            }
        };
        match decode(&bytes, &DecodeOptions::default()) {
            Ok(image) => inputs.push(image),
            Err(error) => {
                eprintln!("{}: decode failed: {error}", path.display());
                return ExitCode::FAILURE;
            }
        }
    }

    let detail = env::var_os("WEBP_RS_BENCH_DETAIL").is_some();
    let started = Instant::now();
    let mut checksum = 0_u64;
    let mut rgba_bytes = 0_usize;
    let mut output_bytes = 0_usize;
    for _ in 0..iterations {
        for (path, image) in paths.iter().zip(&inputs) {
            match encode_lossless_rgba_with_options(image.width, image.height, &image.rgba, options)
            {
                Ok(encoded) => {
                    if detail {
                        println!(
                            "case profile={profile:?} file={} output_bytes={}",
                            path.display(),
                            encoded.len()
                        );
                    }
                    checksum = checksum
                        .wrapping_add(u64::try_from(encoded.len()).unwrap_or(u64::MAX))
                        .wrapping_add(u64::from(encoded.first().copied().unwrap_or(0)));
                    rgba_bytes = rgba_bytes.saturating_add(image.rgba.len());
                    output_bytes = output_bytes.saturating_add(encoded.len());
                    black_box(encoded);
                }
                Err(error) => {
                    eprintln!("encode failed: {error}");
                    return ExitCode::FAILURE;
                }
            }
        }
    }
    let elapsed = started.elapsed();
    let encodes = inputs.len().saturating_mul(iterations);
    println!(
        "encoder=rust profile={} files={} encodes={encodes} rgba_bytes={rgba_bytes} output_bytes={output_bytes} elapsed_ms={:.3} checksum={checksum}",
        match profile {
            LosslessEncodeProfile::Default => "default",
            LosslessEncodeProfile::HighCompression => "high-compression",
            LosslessEncodeProfile::FastDecodeCompact => "fast-decode-compact",
            LosslessEncodeProfile::FastDecodeLowLatency => "fast-decode-low-latency",
            _ => "unknown",
        },
        inputs.len(),
        elapsed.as_secs_f64() * 1_000.0,
    );
    ExitCode::SUCCESS
}
