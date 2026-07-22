//! Direct SharpYUV benchmark over decoded RGBA inputs.
//!
//! Usage: `cargo run --release -p webp --example sharp_yuv_bench -- <iterations> <files...>`

use std::env;
use std::fs;
use std::hint::black_box;
use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

use webp::DecodeOptions;
use webp::decode;
use webp_sharpyuv::SharpYuvPlanes;
use webp_sharpyuv::convert_rgba_to_yuv420;

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
    if let Some(path) = env::var_os("SHARP_YUV_RGBA_CORPUS") {
        if let Err(error) = write_rgba_corpus(PathBuf::from(path), &inputs) {
            eprintln!("cannot write decoded RGBA corpus: {error}");
            return ExitCode::FAILURE;
        }
    }

    let mut checksum = 0_u64;
    let source_checksum = inputs.iter().fold(0_u64, |checksum, image| {
        checksum.wrapping_add(hash_bytes(14_695_981_039_346_656_037_u64, &image.rgba))
    });
    let mut rgba_bytes = 0_usize;
    let started = Instant::now();
    for _ in 0..iterations {
        for image in &inputs {
            let Some(hash) = visible_checksum(image.width, image.height, &image.rgba) else {
                eprintln!("SharpYUV conversion failed");
                return ExitCode::FAILURE;
            };
            checksum = checksum.wrapping_add(hash);
            rgba_bytes = rgba_bytes.saturating_add(image.rgba.len());
            black_box(hash);
        }
    }
    println!(
        "component=rust-sharp-yuv files={} conversions={} rgba_bytes={rgba_bytes} elapsed_ms={:.3} checksum={checksum} source_checksum={source_checksum}",
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

fn visible_checksum(width: u32, height: u32, rgba: &[u8]) -> Option<u64> {
    let width = usize::try_from(width).ok()?;
    let height = usize::try_from(height).ok()?;
    let uv_width = width.div_ceil(2);
    let uv_height = height.div_ceil(2);
    let mut y = vec![0; width.checked_mul(height)?];
    let mut u = vec![0; uv_width.checked_mul(uv_height)?];
    let mut v = vec![0; uv_width.checked_mul(uv_height)?];
    convert_rgba_to_yuv420(
        width.try_into().ok()?,
        height.try_into().ok()?,
        rgba,
        SharpYuvPlanes {
            y_stride: width,
            uv_stride: uv_width,
            y: &mut y,
            u: &mut u,
            v: &mut v,
        },
    )
    .ok()?;
    let mut checksum = 14_695_981_039_346_656_037_u64;
    checksum = hash_bytes(checksum, &(width as u64).to_le_bytes());
    checksum = hash_bytes(checksum, &(height as u64).to_le_bytes());
    checksum = hash_bytes(checksum, &y);
    checksum = hash_bytes(checksum, &u);
    Some(hash_bytes(checksum, &v))
}

fn hash_bytes(mut checksum: u64, bytes: &[u8]) -> u64 {
    for byte in bytes {
        checksum ^= u64::from(*byte);
        checksum = checksum.wrapping_mul(1_099_511_628_211);
    }
    checksum
}

fn write_rgba_corpus(path: PathBuf, images: &[webp::Image]) -> std::io::Result<()> {
    let mut file = fs::File::create(path)?;
    file.write_all(b"SYUVRGBA")?;
    file.write_all(&(images.len() as u32).to_le_bytes())?;
    for image in images {
        file.write_all(&image.width.to_le_bytes())?;
        file.write_all(&image.height.to_le_bytes())?;
        file.write_all(&(image.rgba.len() as u64).to_le_bytes())?;
        file.write_all(&image.rgba)?;
    }
    Ok(())
}
