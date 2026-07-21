//! Release benchmark for the bounded lossless animation encoder profile.

use std::{env, hint::black_box, process::ExitCode, time::Instant};

use webp::{AnimationEncodeFrame, AnimationEncodeOptions, encode_lossless_animation};

const CANVAS_WIDTH: u32 = 320;
const CANVAS_HEIGHT: u32 = 240;

fn main() -> ExitCode {
    let iterations = env::args()
        .nth(1)
        .unwrap_or_else(|| "5".to_owned())
        .parse::<usize>();
    let Ok(iterations) = iterations else {
        eprintln!("usage: animation_encode_bench [positive iterations]");
        return ExitCode::FAILURE;
    };
    if iterations == 0 {
        eprintln!("iterations must be greater than zero");
        return ExitCode::FAILURE;
    }

    let specifications = [
        (0, 0, 320, 240, false, false),
        (80, 60, 160, 120, false, true),
        (0, 0, 64, 64, true, false),
        (256, 176, 64, 64, false, true),
        (0, 80, 320, 80, false, false),
        (96, 56, 128, 128, true, true),
    ];
    let pixels = specifications
        .iter()
        .enumerate()
        .map(|(frame, &(_, _, width, height, _, _))| frame_pixels(width, height, frame as u32))
        .collect::<Vec<_>>();
    let frames = specifications
        .iter()
        .zip(&pixels)
        .enumerate()
        .map(
            |(index, (&(x, y, width, height, dispose_to_background, blend), rgba))| {
                AnimationEncodeFrame {
                    x,
                    y,
                    width,
                    height,
                    duration_ms: 40 + index as u32 * 10,
                    rgba,
                    dispose_to_background,
                    blend,
                }
            },
        )
        .collect::<Vec<_>>();
    let input_bytes = pixels.iter().map(Vec::len).sum::<usize>();
    let options = AnimationEncodeOptions {
        background_rgba: [7, 11, 13, 17],
        loop_count: 3,
    };

    let mut output_bytes = 0_usize;
    let mut checksum = 0_u64;
    let started = Instant::now();
    for _ in 0..iterations {
        let encoded = match encode_lossless_animation(
            CANVAS_WIDTH,
            CANVAS_HEIGHT,
            &frames,
            options,
        ) {
            Ok(encoded) => encoded,
            Err(error) => {
                eprintln!("animation encode failed: {error}");
                return ExitCode::FAILURE;
            }
        };
        output_bytes = output_bytes.saturating_add(encoded.len());
        checksum = checksum
            .wrapping_add(encoded.len() as u64)
            .wrapping_add(u64::from(encoded.first().copied().unwrap_or(0)));
        black_box(encoded);
    }
    let elapsed = started.elapsed();
    println!(
        "encoder=rust profile=vp8l-animation canvas={}x{} frames={} encodes={iterations} rgba_bytes={} output_bytes={output_bytes} elapsed_ms={:.3} checksum={checksum}",
        CANVAS_WIDTH,
        CANVAS_HEIGHT,
        frames.len(),
        input_bytes.saturating_mul(iterations),
        elapsed.as_secs_f64() * 1_000.0,
    );
    ExitCode::SUCCESS
}

fn frame_pixels(width: u32, height: u32, frame: u32) -> Vec<u8> {
    let mut rgba = Vec::with_capacity(width as usize * height as usize * 4);
    for y in 0..height {
        for x in 0..width {
            rgba.extend_from_slice(&[
                (x.wrapping_mul(13) + y.wrapping_mul(3) + frame * 29) as u8,
                (x.wrapping_mul(5) + y.wrapping_mul(11) + frame * 17) as u8,
                (x.wrapping_mul(7) + y.wrapping_mul(19) + frame * 23) as u8,
                if (x + y + frame).is_multiple_of(5) {
                    96
                } else {
                    255
                },
            ]);
        }
    }
    rgba
}
