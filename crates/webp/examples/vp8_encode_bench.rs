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
    let mut quality_bytes = [0_usize; QUALITIES.len()];
    let mut quality_sse = [0_u128; QUALITIES.len()];
    let mut rgb_samples = 0_u128;
    for (quality_index, quality) in QUALITIES.into_iter().enumerate() {
        for image in &inputs {
            let encoded = match encode_lossy_rgba_with_options(
                image.width,
                image.height,
                &image.rgba,
                LossyEncodeOptions { quality },
            ) {
                Ok(encoded) => encoded,
                Err(error) => {
                    eprintln!("quality encode failed: {error}");
                    return ExitCode::FAILURE;
                }
            };
            let decoded = match decode(&encoded, &DecodeOptions::default()) {
                Ok(decoded) => decoded,
                Err(error) => {
                    eprintln!("quality decode failed: {error}");
                    return ExitCode::FAILURE;
                }
            };
            if decoded.rgba.len() != image.rgba.len() {
                eprintln!("quality decode returned a mismatched pixel count");
                return ExitCode::FAILURE;
            }
            quality_bytes[quality_index] =
                quality_bytes[quality_index].saturating_add(encoded.len());
            quality_sse[quality_index] += rgb_sse(&image.rgba, &decoded.rgba);
            if quality_index == 0 {
                rgb_samples += u128::from(image.width) * u128::from(image.height) * 3;
            }
        }
    }
    let quality_psnr = quality_sse.map(|sse| rgb_psnr(sse, rgb_samples));
    let encodes = inputs
        .len()
        .saturating_mul(iterations)
        .saturating_mul(QUALITIES.len());
    println!(
        "encoder=rust profile=vp8-intra16 qualities=0,75,100 files={} encodes={encodes} rgba_bytes={rgba_bytes} output_bytes={output_bytes} elapsed_ms={:.3} checksum={checksum} quality_bytes={},{},{} rgb_sse={},{},{} rgb_psnr={:.3},{:.3},{:.3}",
        inputs.len(),
        elapsed.as_secs_f64() * 1_000.0,
        quality_bytes[0],
        quality_bytes[1],
        quality_bytes[2],
        quality_sse[0],
        quality_sse[1],
        quality_sse[2],
        quality_psnr[0],
        quality_psnr[1],
        quality_psnr[2],
    );
    ExitCode::SUCCESS
}

fn rgb_sse(source: &[u8], decoded: &[u8]) -> u128 {
    source
        .chunks_exact(4)
        .zip(decoded.chunks_exact(4))
        .map(|(source, decoded)| {
            (0..3)
                .map(|channel| {
                    let difference = i32::from(source[channel]) - i32::from(decoded[channel]);
                    (difference * difference) as u128
                })
                .sum::<u128>()
        })
        .sum()
}

fn rgb_psnr(sse: u128, samples: u128) -> f64 {
    if sse == 0 {
        return f64::INFINITY;
    }
    10.0 * (255.0_f64.powi(2) * samples as f64 / sse as f64).log10()
}
