//! Benchmark public container parsing and queries over complete WebP files.
//!
//! Usage: `cargo run --release -p webp-container --example demux_bench -- <iterations> <files...>`

use std::env;
use std::fs;
use std::hint::black_box;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

use webp_container::DemuxOptions;
use webp_container::Demuxer;

fn main() -> ExitCode {
    let Some((iterations, paths)) = arguments() else {
        return ExitCode::FAILURE;
    };
    let mut inputs = Vec::with_capacity(paths.len());
    for path in paths {
        match fs::read(&path) {
            Ok(bytes) => inputs.push(bytes),
            Err(_) => {
                eprintln!("{}: cannot read input", path.display());
                return ExitCode::FAILURE;
            }
        }
    }

    let options = DemuxOptions::default();
    for input in &inputs {
        if let Err(error) = Demuxer::parse(input, &options) {
            eprintln!("input failed demux validation: {error}");
            return ExitCode::FAILURE;
        }
    }

    let started = Instant::now();
    let mut checksum = 0_u64;
    let mut input_bytes = 0_usize;
    for _ in 0..iterations {
        for input in &inputs {
            let demuxer = match Demuxer::parse(input, &options) {
                Ok(demuxer) => demuxer,
                Err(error) => {
                    eprintln!("demux failed: {error}");
                    return ExitCode::FAILURE;
                }
            };
            checksum = checksum
                .wrapping_add(demuxer.chunk_count() as u64)
                .wrapping_add(demuxer.metadata().iccp.map_or(0, |bytes| bytes.len()) as u64)
                .wrapping_add(demuxer.metadata().exif.map_or(0, |bytes| bytes.len()) as u64)
                .wrapping_add(demuxer.metadata().xmp.map_or(0, |bytes| bytes.len()) as u64)
                .wrapping_add(
                    demuxer
                        .animation()
                        .map_or(0, |animation| animation.frame_count()) as u64,
                )
                .wrapping_add(demuxer.image().map_or(0, |_| 1));
            if let Some(vp8x) = demuxer.vp8x() {
                checksum = checksum
                    .wrapping_add(u64::from(vp8x.canvas_width))
                    .wrapping_add(u64::from(vp8x.canvas_height))
                    .wrapping_add(u64::from(vp8x.flags.bits()));
            }
            input_bytes = input_bytes.saturating_add(input.len());
            black_box(demuxer);
        }
    }

    println!(
        "component=rust-demux files={} parses={} input_bytes={input_bytes} elapsed_ms={:.3} checksum={checksum}",
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
        eprintln!("usage: demux_bench <iterations> <files...>");
        return None;
    }
    Some((iterations, paths))
}
