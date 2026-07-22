//! Reproduces and audits VP8L color-transform streams against both decoders.
//!
//! Usage:
//! `vp8l_color_transform_reproducer scan <m6-dir> <dwebp> [first-failure.webp]`
//! `vp8l_color_transform_reproducer synthetic <dwebp> [stream.webp]`

use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use webp::{DecodeOptions, decode, encode_lossless_rgba};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args_os().skip(1);
    let command = args
        .next()
        .ok_or_else(|| usage().to_string())?
        .to_string_lossy()
        .into_owned();
    let dwebp = path_arg(&mut args)?;
    match command.as_str() {
        "scan" => {
            let input = dwebp;
            let dwebp = path_arg(&mut args)?;
            let save_first = args.next().map(PathBuf::from);
            no_more(&mut args)?;
            scan(&input, &dwebp, save_first.as_deref())
        }
        "synthetic" => {
            let save = args.next().map(PathBuf::from);
            no_more(&mut args)?;
            synthetic(&dwebp, save.as_deref())
        }
        _ => Err(usage().to_string()),
    }
}

fn usage() -> &'static str {
    "usage: vp8l_color_transform_reproducer scan <m6-dir> <dwebp> [first-failure.webp]\n       vp8l_color_transform_reproducer synthetic <dwebp> [stream.webp]"
}

fn scan(input: &Path, dwebp: &Path, save_first: Option<&Path>) -> Result<(), String> {
    let mut paths = fs::read_dir(input)
        .map_err(display_error(input))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .is_some_and(|name| name.to_string_lossy().ends_with("-m6.webp"))
        })
        .collect::<Vec<_>>();
    paths.sort();
    if paths.len() != 102 {
        return Err(format!("{}: expected 102 m6 streams", input.display()));
    }

    println!(
        "id\twidth\theight\trgba_sha256\toutput_sha256\toutput_bytes\tproject_status\tproject_kind\tproject_offset\tproject_context\tdwebp_status\tdwebp_error"
    );
    let mut saved = false;
    for path in paths {
        let id = path
            .file_stem()
            .ok_or_else(|| format!("{}: missing file stem", path.display()))?
            .to_string_lossy()
            .trim_end_matches("-m6")
            .to_string();
        let source_bytes = fs::read(&path).map_err(display_error(&path))?;
        let source = decode(&source_bytes, &DecodeOptions::default())
            .map_err(|error| format!("{} source decode: {error}", path.display()))?;
        let encoded = encode_lossless_rgba(source.width, source.height, &source.rgba)
            .map_err(|error| format!("{id} encode: {error}"))?;
        let (project_status, project_kind, project_offset, project_context) =
            match decode(&encoded, &DecodeOptions::default()) {
                Ok(decoded) if decoded.rgba == source.rgba => (
                    "ok".to_string(),
                    String::new(),
                    String::new(),
                    String::new(),
                ),
                Ok(_) => (
                    "rgba_mismatch".to_string(),
                    String::new(),
                    String::new(),
                    String::new(),
                ),
                Err(error) => (
                    "error".to_string(),
                    format!("{:?}", error.kind()),
                    error
                        .offset()
                        .map_or_else(String::new, |value| value.to_string()),
                    error.context().to_string(),
                ),
            };
        let (dwebp_status, dwebp_error) = oracle_status(dwebp, &encoded, &source.rgba)?;
        let failed = project_status != "ok" || dwebp_status != "ok";
        if failed
            && !saved
            && let Some(destination) = save_first
        {
            fs::write(destination, &encoded).map_err(display_error(destination))?;
            saved = true;
        }
        println!(
            "{id}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            source.width,
            source.height,
            sha256(&source.rgba)?,
            sha256(&encoded)?,
            encoded.len(),
            project_status,
            project_kind,
            project_offset,
            clean(&project_context),
            dwebp_status,
            clean(&dwebp_error),
        );
    }
    Ok(())
}

fn synthetic(dwebp: &Path, save: Option<&Path>) -> Result<(), String> {
    const WIDTH: u32 = 129;
    const HEIGHT: u32 = 129;
    let mut rgba = Vec::with_capacity((WIDTH * HEIGHT * 4) as usize);
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let green = x.wrapping_add(y.wrapping_mul(3)) as u8;
            rgba.extend_from_slice(&[green.wrapping_add(3), green, green.wrapping_sub(5), u8::MAX]);
        }
    }
    let encoded = encode_lossless_rgba(WIDTH, HEIGHT, &rgba)
        .map_err(|error| format!("synthetic encode: {error}"))?;
    if let Some(destination) = save {
        fs::write(destination, &encoded).map_err(display_error(destination))?;
    }
    println!("width\t{WIDTH}");
    println!("height\t{HEIGHT}");
    println!("rgba_sha256\t{}", sha256(&rgba)?);
    println!("output_sha256\t{}", sha256(&encoded)?);
    println!("output_bytes\t{}", encoded.len());
    match decode(&encoded, &DecodeOptions::default()) {
        Ok(decoded) => println!("project_status\trgba_equal={}", decoded.rgba == rgba),
        Err(error) => println!(
            "project_status\terror kind={:?} offset={:?} context={}",
            error.kind(),
            error.offset(),
            error.context()
        ),
    }
    let (status, error) = oracle_status(dwebp, &encoded, &rgba)?;
    println!("dwebp_status\t{status}");
    if !error.is_empty() {
        println!("dwebp_error\t{}", clean(&error));
    }
    Ok(())
}

fn oracle_status(
    dwebp: &Path,
    encoded: &[u8],
    expected: &[u8],
) -> Result<(String, String), String> {
    let scratch = ScratchDirectory::new()?;
    let webp_path = scratch.0.join("input.webp");
    let pam_path = scratch.0.join("output.pam");
    fs::write(&webp_path, encoded).map_err(display_error(&webp_path))?;
    let output = Command::new(dwebp)
        .arg(&webp_path)
        .arg("-pam")
        .arg("-o")
        .arg(&pam_path)
        .output()
        .map_err(|error| format!("{}: {error}", dwebp.display()))?;
    if !output.status.success() {
        return Ok((
            "error".to_string(),
            String::from_utf8_lossy(&output.stderr).into_owned(),
        ));
    }
    let actual = pam_rgba(&pam_path)?;
    Ok((
        if actual == expected {
            "ok".to_string()
        } else {
            "rgba_mismatch".to_string()
        },
        String::new(),
    ))
}

fn pam_rgba(path: &Path) -> Result<Vec<u8>, String> {
    let pam = fs::read(path).map_err(display_error(path))?;
    let marker = b"ENDHDR\n";
    let start = pam
        .windows(marker.len())
        .position(|window| window == marker)
        .map(|offset| offset + marker.len())
        .ok_or_else(|| format!("{}: PAM header terminator missing", path.display()))?;
    Ok(pam[start..].to_vec())
}

fn sha256(bytes: &[u8]) -> Result<String, String> {
    let mut child = Command::new("shasum")
        .args(["-a", "256"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|error| format!("spawn shasum: {error}"))?;
    child
        .stdin
        .take()
        .ok_or_else(|| "open shasum stdin".to_string())?
        .write_all(bytes)
        .map_err(|error| format!("write shasum stdin: {error}"))?;
    let output = child
        .wait_with_output()
        .map_err(|error| format!("wait for shasum: {error}"))?;
    if !output.status.success() {
        return Err(format!("shasum exited with {}", output.status));
    }
    String::from_utf8(output.stdout)
        .map_err(|error| format!("shasum output: {error}"))?
        .split_whitespace()
        .next()
        .map(str::to_string)
        .ok_or_else(|| "shasum returned no digest".to_string())
}

fn path_arg(args: &mut impl Iterator<Item = std::ffi::OsString>) -> Result<PathBuf, String> {
    args.next()
        .map(PathBuf::from)
        .ok_or_else(|| usage().to_string())
}

fn no_more(args: &mut impl Iterator<Item = std::ffi::OsString>) -> Result<(), String> {
    if args.next().is_some() {
        return Err(usage().to_string());
    }
    Ok(())
}

fn clean(value: &str) -> String {
    value.replace(['\t', '\r', '\n'], " ").trim().to_string()
}

fn display_error(path: &Path) -> impl FnOnce(std::io::Error) -> String + '_ {
    move |error| format!("{}: {error}", path.display())
}

struct ScratchDirectory(PathBuf);

impl ScratchDirectory {
    fn new() -> Result<Self, String> {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| format!("system time: {error}"))?
            .as_nanos();
        let path = env::temp_dir().join(format!(
            "webp-color-transform-reproducer-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir(&path).map_err(display_error(&path))?;
        Ok(Self(path))
    }
}

impl Drop for ScratchDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
