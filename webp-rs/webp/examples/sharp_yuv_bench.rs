//! Direct private-SharpYUV benchmark over decoded RGBA inputs.
//!
//! Usage: `cargo run --release -p webp --example sharp_yuv_bench --features fuzzing -- <iterations> <files...>`

use std::env;
use std::fs;
use std::hint::black_box;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

use webp::DecodeOptions;
use webp::decode;
use webp::fuzzing::sharp_yuv420_visible_checksum;

fn main() -> ExitCode {
    let Some((iterations, paths)) = arguments() else {
        return ExitCode::FAILURE;
    };
    let mut inputs = Vec::with_capacity(paths.len());
    for path in paths {
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(_) => {
                eprintln!("{}: cannot read input", path.display());
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

    let started = Instant::now();
    let mut checksum = 0_u64;
    let mut rgba_bytes = 0_usize;
    for _ in 0..iterations {
        for image in &inputs {
            let Some(hash) = sharp_yuv420_visible_checksum(image.width, image.height, &image.rgba)
            else {
                eprintln!("SharpYUV conversion failed");
                return ExitCode::FAILURE;
            };
            checksum = checksum.wrapping_add(hash);
            rgba_bytes = rgba_bytes.saturating_add(image.rgba.len());
            black_box(hash);
        }
    }
    println!(
        "component=rust-sharp-yuv files={} conversions={} rgba_bytes={rgba_bytes} elapsed_ms={:.3} checksum={checksum}",
        inputs.len(),
        inputs.len().saturating_mul(iterations),
        started.elapsed().as_secs_f64() * 1_000.0,
    );
    ExitCode::SUCCESS
}

fn arguments() -> Option<(usize, Vec<PathBuf>)> {
    let mut arguments = env::args_os().skip(1);
    let iterations = arguments.next()?.to_string_lossy().parse().ok()?;
    if iterations == 0 {
        eprintln!("iterations must be greater than zero");
        return None;
    }
    let paths = arguments.map(PathBuf::from).collect::<Vec<_>>();
    if paths.is_empty() {
        eprintln!("usage: sharp_yuv_bench <iterations> <files...>");
        return None;
    }
    Some((iterations, paths))
}
