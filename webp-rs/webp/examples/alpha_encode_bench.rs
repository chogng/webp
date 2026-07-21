//! End-to-end lossy-RGB/lossless-ALPH encoder benchmark.

use std::env;
use std::fs;
use std::hint::black_box;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

use webp::AlphaCompression;
use webp::AlphaEncodeOptions;
use webp::AlphaFilterSelection;
use webp::DecodeOptions;
use webp::LossyEncodeOptions;
use webp::decode;
use webp::encode_lossy_rgba_with_alpha_options;

fn main() -> ExitCode {
    let mut arguments = env::args_os().skip(1);
    let Some(iterations) = arguments.next() else {
        eprintln!("usage: alpha_encode_bench <iterations> <files...>");
        return ExitCode::FAILURE;
    };
    let Ok(iterations) = iterations.to_string_lossy().parse::<usize>() else {
        eprintln!("iterations must be a positive integer");
        return ExitCode::FAILURE;
    };
    let paths = arguments.map(PathBuf::from).collect::<Vec<_>>();
    if iterations == 0 || paths.is_empty() {
        eprintln!("provide a positive iteration count and at least one input");
        return ExitCode::FAILURE;
    }

    let mut images = Vec::new();
    for path in &paths {
        let bytes = match fs::read(path) {
            Ok(bytes) => bytes,
            Err(error) => {
                eprintln!("{}: {error}", path.display());
                return ExitCode::FAILURE;
            }
        };
        match decode(&bytes, &DecodeOptions::default()) {
            Ok(image) if image.rgba.chunks_exact(4).any(|pixel| pixel[3] != 255) => {
                images.push(image);
            }
            Ok(_) => {
                eprintln!("{}: input has no transparency", path.display());
                return ExitCode::FAILURE;
            }
            Err(error) => {
                eprintln!("{}: {error}", path.display());
                return ExitCode::FAILURE;
            }
        }
    }

    let alpha_options = AlphaEncodeOptions {
        compression: AlphaCompression::Lossless,
        filter: AlphaFilterSelection::Fast,
        quality: 100,
    };
    let started = Instant::now();
    let mut checksum = 0_u64;
    let mut rgba_bytes = 0_usize;
    let mut output_bytes = 0_usize;
    let mut alpha_bytes = 0_usize;
    for _ in 0..iterations {
        for image in &images {
            let encoded = match encode_lossy_rgba_with_alpha_options(
                image.width,
                image.height,
                &image.rgba,
                LossyEncodeOptions { quality: 75 },
                alpha_options,
            ) {
                Ok(encoded) => encoded,
                Err(error) => {
                    eprintln!("encode failed: {error}");
                    return ExitCode::FAILURE;
                }
            };
            checksum = checksum
                .wrapping_add(encoded.len() as u64)
                .wrapping_add(u64::from(encoded[0]));
            rgba_bytes = rgba_bytes.saturating_add(image.rgba.len());
            output_bytes = output_bytes.saturating_add(encoded.len());
            alpha_bytes = alpha_bytes.saturating_add(alpha_payload_len(&encoded));
            black_box(encoded);
        }
    }
    println!(
        "encoder=rust profile=vp8-q75-alpha-lossless-fast files={} encodes={} rgba_bytes={rgba_bytes} output_bytes={output_bytes} alpha_bytes={alpha_bytes} elapsed_ms={:.3} checksum={checksum}",
        images.len(),
        images.len().saturating_mul(iterations),
        started.elapsed().as_secs_f64() * 1_000.0,
    );
    ExitCode::SUCCESS
}

fn alpha_payload_len(data: &[u8]) -> usize {
    let mut offset = 12_usize;
    while offset + 8 <= data.len() {
        let size = u32::from_le_bytes(data[offset + 4..offset + 8].try_into().unwrap()) as usize;
        if &data[offset..offset + 4] == b"ALPH" {
            return size;
        }
        offset = offset.saturating_add(8 + size + (size & 1));
    }
    0
}
