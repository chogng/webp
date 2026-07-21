//! End-to-end decoder benchmark for a fixed set of WebP files.
//!
//! Usage: `cargo run --release -p webp --example decode_bench -- <iterations> <files...>`

use std::env;
use std::fs;
use std::hint::black_box;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

use webp::DecodeOptions;
use webp::decode;

fn main() -> ExitCode {
    let mut arguments = env::args_os().skip(1);
    let Some(iterations) = arguments.next() else {
        eprintln!("usage: decode_bench <iterations> <files...>");
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

    let mut inputs = Vec::with_capacity(paths.len());
    for path in &paths {
        match fs::read(path) {
            Ok(bytes) => inputs.push(bytes),
            Err(error) => {
                eprintln!("{}: {error}", path.display());
                return ExitCode::FAILURE;
            }
        }
    }

    let options = DecodeOptions::default();
    let started = Instant::now();
    let mut checksum = 0_u64;
    let mut rgba_bytes = 0_usize;
    for _ in 0..iterations {
        for input in &inputs {
            match decode(input, &options) {
                Ok(image) => {
                    checksum = checksum
                        .wrapping_add(u64::from(image.width))
                        .wrapping_add(u64::from(image.height))
                        .wrapping_add(u64::from(image.rgba.first().copied().unwrap_or(0)));
                    rgba_bytes = rgba_bytes.saturating_add(image.rgba.len());
                    black_box(&image.rgba);
                }
                Err(error) => {
                    eprintln!("decode failed: {error}");
                    return ExitCode::FAILURE;
                }
            }
        }
    }
    let elapsed = started.elapsed();
    let decodes = inputs.len().saturating_mul(iterations);
    println!(
        "decoder=rust files={} decodes={decodes} rgba_bytes={rgba_bytes} elapsed_ms={:.3} checksum={checksum}",
        inputs.len(),
        elapsed.as_secs_f64() * 1_000.0,
    );
    ExitCode::SUCCESS
}
