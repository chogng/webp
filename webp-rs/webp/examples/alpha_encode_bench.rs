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
use webp_alpha::encode as encode_alpha;

struct BenchImage {
    name: String,
    width: u32,
    height: u32,
    rgba: Vec<u8>,
    alpha: Vec<u8>,
}

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
                let alpha = image.rgba.chunks_exact(4).map(|pixel| pixel[3]).collect();
                images.push(BenchImage {
                    name: path
                        .file_name()
                        .unwrap_or(path.as_os_str())
                        .to_string_lossy()
                        .into_owned(),
                    width: image.width,
                    height: image.height,
                    rgba: image.rgba,
                    alpha,
                });
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
        let (alpha_size, alpha_header) = alpha_payload(&encoded);
        let distinct_alpha = distinct_alpha_count(&image.alpha);
        let transparent = image.alpha.iter().filter(|&&alpha| alpha == 0).count();
        let translucent = image
            .alpha
            .iter()
            .filter(|&&alpha| alpha != 0 && alpha != u8::MAX)
            .count();
        println!(
            "case encoder=rust file={} width={} height={} pixels={} distinct_alpha={distinct_alpha} transparent_pixels={transparent} translucent_pixels={translucent} alpha_compression={} alpha_filter={} output_bytes={} alpha_bytes={alpha_size} alpha_bpp={:.6} alpha_raw_ratio={:.6}",
            image.name,
            image.width,
            image.height,
            image.alpha.len(),
            alpha_header & 0b11,
            (alpha_header >> 2) & 0b11,
            encoded.len(),
            bits_per_pixel(alpha_size, image.alpha.len()),
            ratio(alpha_size, image.alpha.len()),
        );
    }

    let mut whole_checksum = 0_u64;
    let mut whole_rgba_bytes = 0_usize;
    let mut whole_output_bytes = 0_usize;
    let mut whole_alpha_bytes = 0_usize;
    let mut whole_elapsed_ms = 0.0_f64;
    for image in &images {
        let started = Instant::now();
        let mut image_output_bytes = 0_usize;
        let mut image_alpha_bytes = 0_usize;
        for _ in 0..iterations {
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
            whole_checksum = whole_checksum
                .wrapping_add(encoded.len() as u64)
                .wrapping_add(u64::from(encoded[0]));
            whole_rgba_bytes = whole_rgba_bytes.saturating_add(image.rgba.len());
            whole_output_bytes = whole_output_bytes.saturating_add(encoded.len());
            let encoded_alpha_bytes = alpha_payload(&encoded).0;
            whole_alpha_bytes = whole_alpha_bytes.saturating_add(encoded_alpha_bytes);
            image_output_bytes = image_output_bytes.saturating_add(encoded.len());
            image_alpha_bytes = image_alpha_bytes.saturating_add(encoded_alpha_bytes);
            black_box(encoded);
        }
        let elapsed_ms = started.elapsed().as_secs_f64() * 1_000.0;
        whole_elapsed_ms += elapsed_ms;
        println!(
            "measurement encoder=rust profile=vp8-q75-alpha-lossless-fast file={} encodes={iterations} pixels={} output_bytes={image_output_bytes} alpha_bytes={image_alpha_bytes} elapsed_ms={elapsed_ms:.3} mpix_s={:.3} ns_pixel={:.3}",
            image.name,
            image.alpha.len().saturating_mul(iterations),
            throughput(image.alpha.len(), iterations, elapsed_ms),
            nanoseconds_per_pixel(image.alpha.len(), iterations, elapsed_ms),
        );
    }
    println!(
        "aggregate encoder=rust profile=vp8-q75-alpha-lossless-fast files={} encodes={} pixels={} rgba_bytes={whole_rgba_bytes} output_bytes={whole_output_bytes} alpha_bytes={whole_alpha_bytes} elapsed_ms={whole_elapsed_ms:.3} mpix_s={:.3} ns_pixel={:.3} checksum={whole_checksum}",
        images.len(),
        images.len().saturating_mul(iterations),
        images
            .iter()
            .map(|image| image.alpha.len())
            .sum::<usize>()
            .saturating_mul(iterations),
        aggregate_throughput(&images, iterations, whole_elapsed_ms),
        aggregate_nanoseconds_per_pixel(&images, iterations, whole_elapsed_ms),
    );

    let mut alpha_checksum = 0_u64;
    let mut alpha_input_bytes = 0_usize;
    let mut alpha_output_bytes = 0_usize;
    let mut alpha_elapsed_ms = 0.0_f64;
    for image in &images {
        let started = Instant::now();
        let mut image_output_bytes = 0_usize;
        for _ in 0..iterations {
            let encoded = match encode_alpha(&image.alpha, image.width, image.height, alpha_options)
            {
                Ok(encoded) => encoded,
                Err(error) => {
                    eprintln!("alpha encode failed: {error}");
                    return ExitCode::FAILURE;
                }
            };
            alpha_checksum = alpha_checksum
                .wrapping_add(encoded.len() as u64)
                .wrapping_add(u64::from(encoded[0]));
            alpha_input_bytes = alpha_input_bytes.saturating_add(image.alpha.len());
            alpha_output_bytes = alpha_output_bytes.saturating_add(encoded.len());
            image_output_bytes = image_output_bytes.saturating_add(encoded.len());
            black_box(encoded);
        }
        let elapsed_ms = started.elapsed().as_secs_f64() * 1_000.0;
        alpha_elapsed_ms += elapsed_ms;
        println!(
            "measurement encoder=rust profile=alpha-only-lossless-fast file={} encodes={iterations} pixels={} alpha_bytes={image_output_bytes} elapsed_ms={elapsed_ms:.3} mpix_s={:.3} ns_pixel={:.3}",
            image.name,
            image.alpha.len().saturating_mul(iterations),
            throughput(image.alpha.len(), iterations, elapsed_ms),
            nanoseconds_per_pixel(image.alpha.len(), iterations, elapsed_ms),
        );
    }
    println!(
        "aggregate encoder=rust profile=alpha-only-lossless-fast files={} encodes={} pixels={} alpha_input_bytes={alpha_input_bytes} alpha_bytes={alpha_output_bytes} elapsed_ms={alpha_elapsed_ms:.3} mpix_s={:.3} ns_pixel={:.3} checksum={alpha_checksum}",
        images.len(),
        images.len().saturating_mul(iterations),
        images
            .iter()
            .map(|image| image.alpha.len())
            .sum::<usize>()
            .saturating_mul(iterations),
        aggregate_throughput(&images, iterations, alpha_elapsed_ms),
        aggregate_nanoseconds_per_pixel(&images, iterations, alpha_elapsed_ms),
    );
    ExitCode::SUCCESS
}

fn distinct_alpha_count(alpha: &[u8]) -> usize {
    let mut seen = [false; 256];
    for &sample in alpha {
        seen[usize::from(sample)] = true;
    }
    seen.into_iter().filter(|value| *value).count()
}

fn bits_per_pixel(bytes: usize, pixels: usize) -> f64 {
    bytes as f64 * 8.0 / pixels as f64
}

fn ratio(bytes: usize, input_bytes: usize) -> f64 {
    bytes as f64 / input_bytes as f64
}

fn throughput(pixels: usize, iterations: usize, elapsed_ms: f64) -> f64 {
    pixels.saturating_mul(iterations) as f64 / elapsed_ms / 1_000.0
}

fn nanoseconds_per_pixel(pixels: usize, iterations: usize, elapsed_ms: f64) -> f64 {
    elapsed_ms * 1_000_000.0 / pixels.saturating_mul(iterations) as f64
}

fn aggregate_throughput(images: &[BenchImage], iterations: usize, elapsed_ms: f64) -> f64 {
    images
        .iter()
        .map(|image| image.alpha.len())
        .sum::<usize>()
        .saturating_mul(iterations) as f64
        / elapsed_ms
        / 1_000.0
}

fn aggregate_nanoseconds_per_pixel(
    images: &[BenchImage],
    iterations: usize,
    elapsed_ms: f64,
) -> f64 {
    elapsed_ms * 1_000_000.0
        / images
            .iter()
            .map(|image| image.alpha.len())
            .sum::<usize>()
            .saturating_mul(iterations) as f64
}

fn alpha_payload(data: &[u8]) -> (usize, u8) {
    let mut offset = 12_usize;
    while offset + 8 <= data.len() {
        let size = u32::from_le_bytes(data[offset + 4..offset + 8].try_into().unwrap()) as usize;
        if &data[offset..offset + 4] == b"ALPH" {
            return (size, data.get(offset + 8).copied().unwrap_or(0));
        }
        offset = offset.saturating_add(8 + size + (size & 1));
    }
    (0, 0)
}
