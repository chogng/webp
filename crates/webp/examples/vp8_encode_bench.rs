//! End-to-end static VP8 lossy encoder benchmark over a fixed quality matrix.
//!
//! Usage: `cargo run --release -p webp --example vp8_encode_bench -- <iterations> <files...>`

use std::{env, fs, hint::black_box, path::PathBuf, process::ExitCode, time::Instant};

use webp::{DecodeOptions, LossyEncodeOptions, decode, encode_lossy_rgba_with_options};

const QUALITIES: [u8; 3] = [0, 75, 100];

fn main() -> ExitCode {
    let mut arguments = env::args_os().skip(1);
    let Some(iterations) = arguments.next() else {
        eprintln!("usage: vp8_encode_bench <iterations> <files...>");
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
        let Ok(bytes) = fs::read(path) else {
            eprintln!("{}: cannot read input", path.display());
            return ExitCode::FAILURE;
        };
        match decode(&bytes, &DecodeOptions::default()) {
            Ok(image) => inputs.push(image),
            Err(error) => {
                eprintln!("{}: decode failed: {error}", path.display());
                return ExitCode::FAILURE;
            }
        }
    }

    let started = Instant::now();
    let mut checksum = 0_u64;
    let mut rgba_bytes = 0_usize;
    let mut output_bytes = 0_usize;
    for _ in 0..iterations {
        for quality in QUALITIES {
            for image in &inputs {
                match encode_lossy_rgba_with_options(
                    image.width,
                    image.height,
                    &image.rgba,
                    LossyEncodeOptions { quality },
                ) {
                    Ok(encoded) => {
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
    }
    let elapsed = started.elapsed();
    let encodes = inputs
        .len()
        .saturating_mul(iterations)
        .saturating_mul(QUALITIES.len());
    println!(
        "encoder=rust profile=vp8-intra16 qualities=0,75,100 files={} encodes={encodes} rgba_bytes={rgba_bytes} output_bytes={output_bytes} elapsed_ms={:.3} checksum={checksum}",
        inputs.len(),
        elapsed.as_secs_f64() * 1_000.0,
    );
    ExitCode::SUCCESS
}
