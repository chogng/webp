//! Product validation reproducer for VP8L image-writer profiles.

use std::fs;
use std::hint::black_box;
use std::path::{Path, PathBuf};
use std::time::Instant;

use super::*;
use crate::{
    LosslessEncodeOptions, LosslessEncodeProfile, encode_lossless_rgba,
    encode_lossless_rgba_with_options,
};
use webp_decode::{DecodeOptions, decode};

struct Source {
    id: String,
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

#[derive(Clone, Copy)]
enum Layout {
    Default,
    Single,
    Compact,
    LowLatency,
    CompactControl,
    LowLatencyControl,
    CompactWriterControl,
    LowLatencyWriterControl,
    LibwebpM6,
}

#[test]
#[ignore = "invoked explicitly by the product validation reproducer"]
fn product_validation_reproducer() {
    run().unwrap_or_else(|error| panic!("product validation reproducer: {error}"));
}

fn run() -> Result<(), String> {
    let command = environment("VP8L_PRODUCT_COMMAND")?;
    let input = PathBuf::from(environment("VP8L_PRODUCT_INPUT")?);
    match command.as_str() {
        "generate" => generate(&input, Path::new(&environment("VP8L_PRODUCT_OUTPUT")?)),
        "bench-encode" => bench_encode(
            &input,
            parse_layout(&environment("VP8L_PRODUCT_LAYOUT")?)?,
            &environment("VP8L_PRODUCT_ROUND")?,
        ),
        "bench-decode" => bench_decode(
            &input,
            parse_layout(&environment("VP8L_PRODUCT_LAYOUT")?)?,
            &environment("VP8L_PRODUCT_ROUND")?,
        ),
        "audit-exact" => audit_exact(&input),
        _ => Err(format!("unsupported command {command}")),
    }
}

fn generate(input: &Path, output: &Path) -> Result<(), String> {
    let layouts = [
        Layout::Default,
        Layout::Single,
        Layout::Compact,
        Layout::LowLatency,
    ];
    fs::create_dir_all(output.join("expected")).map_err(display_error(output))?;
    for layout in layouts {
        fs::create_dir_all(output.join(layout_name(layout))).map_err(display_error(output))?;
    }
    println!("stream\tid\tlayout\tbytes\trgba_hash\tstream_hash\tencode_ns\tproject_exact");
    let start = optional_usize("VP8L_PRODUCT_START")?.unwrap_or(0);
    let limit = optional_usize("VP8L_PRODUCT_LIMIT")?.unwrap_or(usize::MAX);
    for path in input_paths(input)?.into_iter().skip(start).take(limit) {
        let source = read_source(&path)?;
        let expected_path = output.join("expected").join(format!("{}.rgba", source.id));
        fs::write(&expected_path, &source.rgba).map_err(display_error(&expected_path))?;
        for layout in layouts {
            let started = Instant::now();
            let encoded = encode_layout(&source, layout)?;
            let encode_ns = started.elapsed().as_nanos();
            let image = decode(&encoded, &DecodeOptions::default())
                .map_err(|error| format!("{} {}: {error}", source.id, layout_name(layout)))?;
            let exact = image.width == source.width
                && image.height == source.height
                && image.rgba == source.rgba;
            if !exact {
                return Err(format!(
                    "{} {}: project decoder mismatch",
                    source.id,
                    layout_name(layout)
                ));
            }
            let stream_path = output
                .join(layout_name(layout))
                .join(format!("{}.webp", source.id));
            fs::write(&stream_path, &encoded).map_err(display_error(&stream_path))?;
            println!(
                "stream\t{}\t{}\t{}\t{:016x}\t{:016x}\t{}\t1",
                source.id,
                layout_name(layout),
                encoded.len(),
                fnv1a(&source.rgba),
                fnv1a(&encoded),
                encode_ns
            );
        }
    }
    Ok(())
}

fn bench_encode(input: &Path, layout: Layout, round: &str) -> Result<(), String> {
    let sources = input_paths(input)?
        .iter()
        .map(|path| read_source(path))
        .collect::<Result<Vec<_>, _>>()?;
    let started = Instant::now();
    let mut checksum = 0_u64;
    let mut output_bytes = 0_usize;
    for source in &sources {
        let item_started = Instant::now();
        let encoded = encode_layout(source, layout)?;
        let elapsed = item_started.elapsed().as_nanos();
        let hash = fnv1a(&encoded);
        checksum ^= hash.rotate_left((source.id.len() % 64) as u32);
        output_bytes = output_bytes.saturating_add(encoded.len());
        println!(
            "measurement\tencode\t{round}\t{}\t{}\t{elapsed}\t{}\t{}\t{hash:016x}",
            layout_name(layout),
            source.id,
            source.rgba.len(),
            encoded.len()
        );
        black_box(encoded);
    }
    println!(
        "aggregate\tencode\t{round}\t{}\t{}\t{}\t{}\t{output_bytes}\t{checksum:016x}",
        layout_name(layout),
        sources.len(),
        started.elapsed().as_nanos(),
        sources
            .iter()
            .map(|source| source.rgba.len())
            .sum::<usize>()
    );
    Ok(())
}

fn bench_decode(input: &Path, layout: Layout, round: &str) -> Result<(), String> {
    let directory = input.join(layout_name(layout));
    let paths = stream_paths(&directory)?;
    let inputs = paths
        .iter()
        .map(|path| fs::read(path).map_err(display_error(path)))
        .collect::<Result<Vec<_>, _>>()?;
    let started = Instant::now();
    let mut checksum = 0_u64;
    let mut rgba_bytes = 0_usize;
    for (path, bytes) in paths.iter().zip(&inputs) {
        let item_started = Instant::now();
        let image = decode(bytes, &DecodeOptions::default())
            .map_err(|error| format!("{}: {error}", path.display()))?;
        let hash = fnv1a(&image.rgba);
        let elapsed = item_started.elapsed().as_nanos();
        rgba_bytes = rgba_bytes.saturating_add(image.rgba.len());
        checksum ^= hash.rotate_left((rgba_bytes % 64) as u32);
        println!(
            "measurement\tdecode\t{round}\t{}\t{}\t{elapsed}\t{}\t{}\t{hash:016x}",
            layout_name(layout),
            file_id(path)?,
            image.rgba.len(),
            bytes.len()
        );
        black_box(image);
    }
    println!(
        "aggregate\tdecode\t{round}\t{}\t{}\t{}\t{rgba_bytes}\t{}\t{checksum:016x}",
        layout_name(layout),
        inputs.len(),
        started.elapsed().as_nanos(),
        inputs.iter().map(Vec::len).sum::<usize>()
    );
    Ok(())
}

fn audit_exact(input: &Path) -> Result<(), String> {
    println!(
        "exact\tid\tprofile\tbytes\tstream_hash\tpredicted_bits\twritten_bits\tpredicted_payload_bytes\tpredicted_riff_bytes\tsingle_actual_riff_bytes\testimate_exact\tlosing_single_main_written\testimator_fallback\tcandidate_won\tcontrol_exact"
    );
    for path in input_paths(input)? {
        let source = read_source(&path)?;
        let (predicted_bits, written_bits, predicted_payload_bytes, predicted_riff_bytes) =
            spatial_writer::single_estimate_for_test(source.width, source.height, &source.rgba)
                .map_err(|error| format!("{} estimate: {error}", source.id))?;
        let single =
            spatial_writer::encode_single_for_test(source.width, source.height, &source.rgba)
                .map_err(|error| format!("{} single: {error}", source.id))?;
        let actual_payload_bytes = u32::from_le_bytes(
            single[16..20]
                .try_into()
                .map_err(|_| format!("{}: invalid single RIFF", source.id))?,
        ) as usize;
        if predicted_bits != written_bits
            || predicted_payload_bytes != actual_payload_bytes
            || predicted_riff_bytes != single.len()
        {
            return Err(format!("{}: exact single estimate mismatch", source.id));
        }
        for (name, profile) in [
            ("compact", spatial_plan::SpatialProfile::Compact),
            ("low-latency", spatial_plan::SpatialProfile::LowLatency),
        ] {
            let control = spatial_writer::encode_profile_control_for_test(
                source.width,
                source.height,
                &source.rgba,
                profile,
            )
            .map_err(|error| format!("{} {name} control: {error}", source.id))?;
            let (exact, stats) = spatial_writer::encode_profile_exact_for_test(
                source.width,
                source.height,
                &source.rgba,
                profile,
            )
            .map_err(|error| format!("{} {name} exact: {error}", source.id))?;
            let control_exact = control == exact;
            if !control_exact {
                return Err(format!("{} {name}: control/exact mismatch", source.id));
            }
            println!(
                "exact\t{}\t{name}\t{}\t{:016x}\t{}\t{}\t{}\t{}\t{}\t1\t{}\t{}\t{}\t{}",
                source.id,
                exact.len(),
                fnv1a(&exact),
                stats.predicted_payload_bits.unwrap_or_default(),
                written_bits,
                stats.predicted_payload_bytes.unwrap_or_default(),
                stats.predicted_riff_bytes.unwrap_or_default(),
                single.len(),
                u8::from(stats.losing_single_main_written),
                u8::from(stats.estimator_fallback),
                u8::from(stats.candidate_won),
                u8::from(control_exact),
            );
        }
    }
    Ok(())
}

fn encode_layout(source: &Source, layout: Layout) -> Result<Vec<u8>, String> {
    let result = match layout {
        Layout::Default => encode_lossless_rgba(source.width, source.height, &source.rgba),
        Layout::Single => {
            spatial_writer::encode_single_for_test(source.width, source.height, &source.rgba)
        }
        Layout::Compact | Layout::LowLatency => {
            let options = LosslessEncodeOptions {
                profile: match layout {
                    Layout::Compact => LosslessEncodeProfile::FastDecodeCompact,
                    Layout::LowLatency => LosslessEncodeProfile::FastDecodeLowLatency,
                    Layout::Default
                    | Layout::Single
                    | Layout::CompactControl
                    | Layout::LowLatencyControl
                    | Layout::CompactWriterControl
                    | Layout::LowLatencyWriterControl
                    | Layout::LibwebpM6 => unreachable!(),
                },
            };
            encode_lossless_rgba_with_options(source.width, source.height, &source.rgba, options)
        }
        Layout::CompactControl | Layout::LowLatencyControl => {
            let profile = match layout {
                Layout::CompactControl => spatial_plan::SpatialProfile::Compact,
                Layout::LowLatencyControl => spatial_plan::SpatialProfile::LowLatency,
                _ => unreachable!(),
            };
            spatial_writer::encode_profile_control_for_test(
                source.width,
                source.height,
                &source.rgba,
                profile,
            )
        }
        Layout::CompactWriterControl | Layout::LowLatencyWriterControl => {
            let profile = match layout {
                Layout::CompactWriterControl => spatial_plan::SpatialProfile::Compact,
                Layout::LowLatencyWriterControl => spatial_plan::SpatialProfile::LowLatency,
                _ => unreachable!(),
            };
            encode_profile_writer_control(source, profile)
        }
        Layout::LibwebpM6 => return Err("libwebp m6 is decode-only".to_owned()),
    };
    result.map_err(|error| format!("{} {}: {error}", source.id, layout_name(layout)))
}

/// Reconstructs the latest-main spatial path entirely inside the benchmark
/// harness so the final test binary can compare writers without production
/// feature flags or audit hooks.
fn encode_profile_writer_control(
    source: &Source,
    profile: spatial_plan::SpatialProfile,
) -> Result<Vec<u8>, EncodeError> {
    validate_input(source.width, source.height, &source.rgba)?;
    let width = usize::try_from(source.width).map_err(|_| EncodeError::input_size_overflow())?;
    let stream = TokenStream::collect(&source.rgba, width, true, false, 0)?;
    let single = single_plan::SinglePlan::build(stream.statistics())?;
    let candidate = encode_spatial_writer_control(source, &stream, profile)?;
    if candidate.len() < single.riff_bytes() {
        Ok(candidate)
    } else {
        let mut bits = BitWriter::new();
        write_fast_prefix_control(&mut bits, source)?;
        single.write_main_prefix(&mut bits)?;
        write_tokens_control(&mut bits, stream.tokens(), single.tables())?;
        wrap_vp8l(bits.into_bytes())
    }
}

fn encode_spatial_writer_control(
    source: &Source,
    stream: &TokenStream,
    profile: spatial_plan::SpatialProfile,
) -> Result<Vec<u8>, EncodeError> {
    let plan = spatial_plan::SpatialPlan::build(stream, profile)?;
    let mut bits = BitWriter::new();
    write_fast_prefix_control(&mut bits, source)?;
    write_bits(&mut bits, 0, 1)?;
    write_bits(&mut bits, 1, 1)?;
    write_bits(&mut bits, u32::from(profile.wire_block_bits()), 3)?;
    write_group_map_control(&mut bits, &plan)?;

    let mut tables = Vec::new();
    tables
        .try_reserve_exact(plan.frequencies().len())
        .map_err(|_| EncodeError::allocation_failed())?;
    for frequencies in plan.frequencies() {
        tables.push(write_five_tables_control(&mut bits, frequencies)?);
    }
    let mut pixel = 0_usize;
    for &token in stream.tokens() {
        let group = plan.group_for_pixel(pixel);
        write_token_control(&mut bits, token, &tables[group])?;
        pixel = pixel
            .checked_add(token_stream::token_span(token))
            .ok_or_else(EncodeError::output_size_overflow)?;
    }
    wrap_vp8l(bits.into_bytes())
}

fn write_fast_prefix_control(bits: &mut BitWriter, source: &Source) -> Result<(), EncodeError> {
    write_vp8l_header(
        bits,
        source.width,
        source.height,
        source.rgba.chunks_exact(4).any(|pixel| pixel[3] != u8::MAX),
    )?;
    write_bits(bits, 1, 1)?;
    write_bits(bits, 2, 2)?;
    write_bits(bits, 0, 1)
}

fn write_group_map_control(
    bits: &mut BitWriter,
    plan: &spatial_plan::SpatialPlan,
) -> Result<(), EncodeError> {
    let byte_count = plan
        .group_map()
        .len()
        .checked_mul(4)
        .ok_or_else(EncodeError::output_size_overflow)?;
    let mut rgba = Vec::new();
    rgba.try_reserve_exact(byte_count)
        .map_err(|_| EncodeError::allocation_failed())?;
    for &group in plan.group_map() {
        rgba.extend_from_slice(&[0, group, 0, 0]);
    }
    let stream = TokenStream::collect(&rgba, plan.map_width(), false, false, 0)?;
    write_bits(bits, 0, 1)?;
    let tables = write_five_tables_control(bits, stream.statistics().frequencies())?;
    write_tokens_control(bits, stream.tokens(), &tables)
}

fn write_five_tables_control(
    bits: &mut BitWriter,
    frequencies: &EntropyFrequencies,
) -> Result<EntropyTables, EncodeError> {
    Ok(EntropyTables {
        green: write_adaptive_table(bits, frequencies.green())?,
        red: write_adaptive_table(bits, frequencies.red())?,
        blue: write_adaptive_table(bits, frequencies.blue())?,
        alpha: write_adaptive_table(bits, frequencies.alpha())?,
        distance: write_adaptive_table(bits, frequencies.distance())?,
    })
}

fn write_tokens_control(
    bits: &mut BitWriter,
    tokens: &[EntropyToken],
    tables: &EntropyTables,
) -> Result<(), EncodeError> {
    for &token in tokens {
        write_token_control(bits, token, tables)?;
    }
    Ok(())
}

fn write_token_control(
    bits: &mut BitWriter,
    token: EntropyToken,
    tables: &EntropyTables,
) -> Result<(), EncodeError> {
    match token {
        EntropyToken::Cache(index) => Ok(write_table_symbol(
            bits,
            &tables.green,
            FIRST_CACHE_SYMBOL + index,
        )?),
        EntropyToken::Literal(rgba) => {
            write_table_symbol(bits, &tables.green, usize::from(rgba[1]))?;
            write_table_symbol(bits, &tables.red, usize::from(rgba[0]))?;
            write_table_symbol(bits, &tables.blue, usize::from(rgba[2]))?;
            Ok(write_table_symbol(
                bits,
                &tables.alpha,
                usize::from(rgba[3]),
            )?)
        }
        EntropyToken::Copy { length } => write_lz77_copy(bits, tables, length),
    }
}

fn read_source(path: &Path) -> Result<Source, String> {
    let bytes = fs::read(path).map_err(display_error(path))?;
    let image = decode(&bytes, &DecodeOptions::default())
        .map_err(|error| format!("{}: {error}", path.display()))?;
    Ok(Source {
        id: file_id(path)?
            .strip_suffix("-m6")
            .ok_or_else(|| format!("{}: missing -m6 suffix", path.display()))?
            .to_owned(),
        width: image.width,
        height: image.height,
        rgba: image.rgba,
    })
}

fn input_paths(root: &Path) -> Result<Vec<PathBuf>, String> {
    paths_matching(root, |path| {
        path.file_name()
            .is_some_and(|name| name.to_string_lossy().ends_with("-m6.webp"))
    })
}

fn stream_paths(root: &Path) -> Result<Vec<PathBuf>, String> {
    paths_matching(root, |path| {
        path.extension()
            .is_some_and(|extension| extension == "webp")
    })
}

fn paths_matching(root: &Path, keep: impl Fn(&Path) -> bool) -> Result<Vec<PathBuf>, String> {
    let mut paths = fs::read_dir(root)
        .map_err(display_error(root))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| keep(path))
        .collect::<Vec<_>>();
    paths.sort();
    if paths.is_empty() {
        return Err(format!("{}: no matching inputs", root.display()));
    }
    Ok(paths)
}

fn parse_layout(value: &str) -> Result<Layout, String> {
    match value {
        "default" => Ok(Layout::Default),
        "single" => Ok(Layout::Single),
        "compact" => Ok(Layout::Compact),
        "low-latency" => Ok(Layout::LowLatency),
        "compact-control" => Ok(Layout::CompactControl),
        "low-latency-control" => Ok(Layout::LowLatencyControl),
        "compact-writer-control" => Ok(Layout::CompactWriterControl),
        "low-latency-writer-control" => Ok(Layout::LowLatencyWriterControl),
        "libwebp-m6" => Ok(Layout::LibwebpM6),
        _ => Err(format!("unsupported layout {value}")),
    }
}

const fn layout_name(layout: Layout) -> &'static str {
    match layout {
        Layout::Default => "default",
        Layout::Single => "single",
        Layout::Compact => "compact",
        Layout::LowLatency => "low-latency",
        Layout::CompactControl => "compact-control",
        Layout::LowLatencyControl => "low-latency-control",
        Layout::CompactWriterControl => "compact-writer-control",
        Layout::LowLatencyWriterControl => "low-latency-writer-control",
        Layout::LibwebpM6 => "libwebp-m6",
    }
}

fn file_id(path: &Path) -> Result<String, String> {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .map(str::to_owned)
        .ok_or_else(|| format!("{}: invalid file name", path.display()))
}

fn environment(name: &str) -> Result<String, String> {
    std::env::var(name).map_err(|_| format!("missing {name}"))
}

fn optional_usize(name: &str) -> Result<Option<usize>, String> {
    std::env::var(name)
        .ok()
        .map(|value| {
            value
                .parse::<usize>()
                .map_err(|_| format!("invalid {name}"))
        })
        .transpose()
}

fn display_error(path: &Path) -> impl FnOnce(std::io::Error) -> String + '_ {
    move |error| format!("{}: {error}", path.display())
}

fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100_0000_01b3)
    })
}
