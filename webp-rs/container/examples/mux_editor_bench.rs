//! Benchmark public generic mux reconstruction and unchanged editor round trips.
//!
//! Usage: `cargo run --release -p webp-container --example mux_editor_bench -- <iterations> <files...>`

use std::env;
use std::fs;
use std::hint::black_box;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

use webp_container::DemuxOptions;
use webp_container::Demuxer;
use webp_container::Editor;
use webp_container::MuxChunk;
use webp_container::Muxer;

struct Input {
    bytes: Vec<u8>,
    chunks: Vec<MuxChunk>,
}

fn main() -> ExitCode {
    let Some((iterations, paths)) = arguments() else {
        return ExitCode::FAILURE;
    };
    let options = DemuxOptions::default();
    let mut inputs = Vec::with_capacity(paths.len());
    for path in paths {
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(_) => {
                eprintln!("{}: cannot read input", path.display());
                return ExitCode::FAILURE;
            }
        };
        let demuxer = match Demuxer::parse(&bytes, &options) {
            Ok(demuxer) => demuxer,
            Err(error) => {
                eprintln!("{}: cannot prepare strict input: {error}", path.display());
                return ExitCode::FAILURE;
            }
        };
        let chunks = demuxer
            .chunks()
            .iter()
            .map(|chunk| MuxChunk::new(chunk.fourcc, chunk.payload.to_vec()))
            .collect();
        inputs.push(Input { bytes, chunks });
    }

    let Some((mux_bytes, mux_checksum, mux_elapsed)) = run_mux(iterations, &inputs) else {
        return ExitCode::FAILURE;
    };
    let Some((editor_bytes, editor_checksum, editor_elapsed)) =
        run_editor(iterations, &inputs, &options)
    else {
        return ExitCode::FAILURE;
    };
    println!(
        "component=rust-mux-editor files={} operations={} mux_output_bytes={mux_bytes} mux_elapsed_ms={:.3} mux_checksum={mux_checksum} editor_output_bytes={editor_bytes} editor_elapsed_ms={:.3} editor_checksum={editor_checksum}",
        inputs.len(),
        inputs.len().saturating_mul(iterations),
        mux_elapsed.as_secs_f64() * 1_000.0,
        editor_elapsed.as_secs_f64() * 1_000.0,
    );
    ExitCode::SUCCESS
}

fn run_mux(iterations: usize, inputs: &[Input]) -> Option<(usize, u64, std::time::Duration)> {
    let started = Instant::now();
    let mut output_bytes = 0_usize;
    let mut checksum = 0_u64;
    for _ in 0..iterations {
        for input in inputs {
            let mut muxer = Muxer::new();
            for chunk in &input.chunks {
                muxer.add_chunk(chunk.clone()).ok()?;
            }
            let output = muxer.finish().ok()?;
            output_bytes = output_bytes.saturating_add(output.len());
            checksum = checksum
                .wrapping_add(output.len() as u64)
                .wrapping_add(u64::from(output.first().copied().unwrap_or(0)));
            black_box(output);
        }
    }
    Some((output_bytes, checksum, started.elapsed()))
}

fn run_editor(
    iterations: usize,
    inputs: &[Input],
    options: &DemuxOptions,
) -> Option<(usize, u64, std::time::Duration)> {
    let started = Instant::now();
    let mut output_bytes = 0_usize;
    let mut checksum = 0_u64;
    for _ in 0..iterations {
        for input in inputs {
            let output = Editor::parse(&input.bytes, options).ok()?.finish().ok()?;
            if output != input.bytes {
                eprintln!("unchanged strict editor round trip changed input bytes");
                return None;
            }
            output_bytes = output_bytes.saturating_add(output.len());
            checksum = checksum
                .wrapping_add(output.len() as u64)
                .wrapping_add(u64::from(output.first().copied().unwrap_or(0)));
            black_box(output);
        }
    }
    Some((output_bytes, checksum, started.elapsed()))
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
        eprintln!("usage: mux_editor_bench <iterations> <files...>");
        return None;
    }
    Some((iterations, paths))
}
