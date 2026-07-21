//! Opt-in phase and mode benchmark for the pinned CLIC VP8L cache.
//!
//! Run with `VP8L_PREDICTOR_BENCH_ITERATIONS=3 cargo test --release -p
//! webp-vp8l-literal predictor_phase_clic -- --ignored --nocapture`.

use std::{
    collections::BTreeMap,
    env, fs,
    hint::black_box,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use webp_core::{BitReader, DecodeLimits};
use webp_vp8l::{HEADER_LEN, parse_header};

use super::{
    DecodePhaseTimings, DecodedTransform, decode_no_transform_profiled, inverse_predictor_rgba,
    read_supported_transforms,
};

#[derive(Default)]
struct MethodStats {
    files: usize,
    predictor_files: usize,
    predictor_pixels: u64,
    elapsed: Duration,
    modes: [u64; 14],
}

#[derive(Default)]
struct DecodeStats {
    files: usize,
    rgba_bytes: usize,
    checksum: u64,
    total: Duration,
    phases: DecodePhaseTimings,
}

#[test]
#[ignore = "requires the pinned CLIC VP8L cache"]
fn decode_phases_clic() {
    let root = clic_output_root();
    let iterations = benchmark_iterations();
    let limits = DecodeLimits::default();
    let mut stats = BTreeMap::<u8, DecodeStats>::new();
    let mut paths = webp_paths(&root);
    paths.sort();

    for path in paths {
        let method = method_from_path(&path);
        let container =
            fs::read(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        let data = vp8l_payload(&container)
            .unwrap_or_else(|| panic!("{}: missing VP8L payload", path.display()));
        let summary = stats.entry(method).or_default();
        summary.files += 1;
        for _ in 0..iterations {
            let started = Instant::now();
            let image = decode_no_transform_profiled(data, &limits, &mut summary.phases)
                .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
            summary.total += started.elapsed();
            summary.rgba_bytes += image.rgba.len();
            summary.checksum = summary
                .checksum
                .wrapping_add(u64::from(image.header.width))
                .wrapping_add(u64::from(image.header.height))
                .wrapping_add(u64::from(image.rgba.first().copied().unwrap_or(0)));
            black_box(image);
        }
    }

    eprintln!(
        "decode_phases_clic iterations={iterations} root={}",
        root.display()
    );
    for (method, summary) in stats {
        let measured =
            summary.phases.entropy + summary.phases.rgba_conversion + summary.phases.predictor;
        let other = summary.total.saturating_sub(measured);
        let total_ms = summary.total.as_secs_f64() * 1_000.0;
        eprintln!(
            "method={method} files={} rgba_bytes={} checksum={} total_ms={total_ms:.3} entropy_ms={:.3} entropy_pct={:.1} rgba_conversion_ms={:.3} rgba_conversion_pct={:.1} predictor_ms={:.3} predictor_pct={:.1} other_ms={:.3} other_pct={:.1} literal_pixels={} batched_literals={} cache_hits={} copy_commands={} copy_pixels={} meta_runs={}",
            summary.files,
            summary.rgba_bytes,
            summary.checksum,
            milliseconds(summary.phases.entropy),
            percentage(summary.phases.entropy, summary.total),
            milliseconds(summary.phases.rgba_conversion),
            percentage(summary.phases.rgba_conversion, summary.total),
            milliseconds(summary.phases.predictor),
            percentage(summary.phases.predictor, summary.total),
            milliseconds(other),
            percentage(other, summary.total),
            summary.phases.entropy_paths.literal_pixels,
            summary.phases.entropy_paths.batched_literals,
            summary.phases.entropy_paths.cache_hits,
            summary.phases.entropy_paths.copy_commands,
            summary.phases.entropy_paths.copy_pixels,
            summary.phases.entropy_paths.meta_runs,
        );
    }
}

#[test]
#[ignore = "requires the pinned CLIC VP8L cache"]
fn predictor_phase_clic() {
    let root = clic_output_root();
    let iterations = benchmark_iterations();
    let limits = DecodeLimits::default();
    let mut stats = BTreeMap::<u8, MethodStats>::new();
    let mut paths = webp_paths(&root);
    paths.sort();

    for path in paths {
        let method = method_from_path(&path);
        let container =
            fs::read(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        let data = vp8l_payload(&container)
            .unwrap_or_else(|| panic!("{}: missing VP8L payload", path.display()));
        let header = parse_header(data, &limits)
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        let mut bits = BitReader::with_bit_position(data, HEADER_LEN * 8)
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        let mut budget = limits.work_budget();
        let mut retained_transform_bytes = 0;
        let transforms = read_supported_transforms(
            &mut bits,
            &mut budget,
            &header,
            &limits,
            &mut retained_transform_bytes,
        )
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        let summary = stats.entry(method).or_default();
        summary.files += 1;

        for transform in transforms.transforms {
            let DecodedTransform::Predictor {
                descriptor,
                mode_pixels,
            } = transform
            else {
                continue;
            };
            summary.predictor_files += 1;
            let coverage = mode_coverage(descriptor, &mode_pixels);
            for (mode, pixels) in coverage.into_iter().enumerate() {
                summary.modes[mode] += pixels;
                summary.predictor_pixels += pixels;
            }

            let bytes = usize::try_from(descriptor.image_width)
                .unwrap()
                .checked_mul(usize::try_from(descriptor.image_height).unwrap())
                .unwrap()
                .checked_mul(4)
                .unwrap();
            let original = deterministic_residuals(bytes);
            for _ in 0..iterations {
                let mut pixels = original.clone();
                let started = Instant::now();
                inverse_predictor_rgba(&mut pixels, descriptor, &mode_pixels).unwrap();
                summary.elapsed += started.elapsed();
                black_box(pixels);
            }
        }
    }

    eprintln!(
        "predictor_phase_clic iterations={iterations} root={}",
        root.display()
    );
    for (method, summary) in stats {
        let elapsed_ms = summary.elapsed.as_secs_f64() * 1_000.0;
        let divisor = summary.predictor_pixels as f64 * iterations as f64;
        let ns_per_pixel = summary.elapsed.as_nanos() as f64 / divisor;
        eprintln!(
            "method={method} files={} predictor_files={} predictor_pixels={} elapsed_ms={elapsed_ms:.3} ns_per_pixel={ns_per_pixel:.3} modes={}",
            summary.files,
            summary.predictor_files,
            summary.predictor_pixels,
            format_modes(&summary.modes),
        );
    }
}

fn benchmark_iterations() -> usize {
    env::var("VP8L_PREDICTOR_BENCH_ITERATIONS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|&value| value > 0)
        .unwrap_or(1)
}

fn milliseconds(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1_000.0
}

fn percentage(duration: Duration, total: Duration) -> f64 {
    100.0 * duration.as_secs_f64() / total.as_secs_f64()
}

fn clic_output_root() -> PathBuf {
    env::var_os("VP8L_CLIC_OUTPUT_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .join("third_party/benchdata/clic/vp8l-lossless-exact")
        })
}

fn webp_paths(root: &Path) -> Vec<PathBuf> {
    fs::read_dir(root)
        .unwrap_or_else(|error| panic!("{}: {error}", root.display()))
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "webp")
        })
        .collect()
}

fn vp8l_payload(container: &[u8]) -> Option<&[u8]> {
    if container.len() < 12 || &container[..4] != b"RIFF" || &container[8..12] != b"WEBP" {
        return None;
    }
    let mut offset = 12_usize;
    while offset.checked_add(8)? <= container.len() {
        let chunk = &container[offset..offset + 4];
        let length = usize::try_from(u32::from_le_bytes(
            container[offset + 4..offset + 8].try_into().ok()?,
        ))
        .ok()?;
        let data_start = offset + 8;
        let data_end = data_start.checked_add(length)?;
        if data_end > container.len() {
            return None;
        }
        if chunk == b"VP8L" {
            return Some(&container[data_start..data_end]);
        }
        offset = data_end.checked_add(length & 1)?;
    }
    None
}

fn method_from_path(path: &Path) -> u8 {
    let name = path.file_name().unwrap().to_string_lossy();
    for method in [0, 3, 6] {
        if name.ends_with(&format!("-m{method}.webp")) {
            return method;
        }
    }
    panic!("unknown CLIC method path: {}", path.display());
}

fn mode_coverage(
    descriptor: webp_vp8l::BlockTransformDescriptor,
    mode_pixels: &[u32],
) -> [u64; 14] {
    let width = usize::try_from(descriptor.image_width).unwrap();
    let height = usize::try_from(descriptor.image_height).unwrap();
    let block_size = usize::try_from(descriptor.block_size()).unwrap();
    let mode_width = usize::try_from(descriptor.transform_width).unwrap();
    let mut coverage = [0; 14];
    for y in 1..height {
        let mode_row = (y / block_size) * mode_width;
        let mut x = 1;
        while x < width {
            let mode = ((mode_pixels[mode_row + x / block_size] >> 8) & 0x0f) as usize;
            let x_end = (x & !(block_size - 1))
                .saturating_add(block_size)
                .min(width);
            coverage[mode] += u64::try_from(x_end - x).unwrap();
            x = x_end;
        }
    }
    coverage
}

fn deterministic_residuals(len: usize) -> Vec<u8> {
    let mut state = 0x8d26_4ca5_u32;
    (0..len)
        .map(|_| {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            (state >> 24) as u8
        })
        .collect()
}

fn format_modes(modes: &[u64; 14]) -> String {
    modes
        .iter()
        .enumerate()
        .filter(|(_, pixels)| **pixels != 0)
        .map(|(mode, pixels)| format!("m{mode}:{pixels}"))
        .collect::<Vec<_>>()
        .join(",")
}
