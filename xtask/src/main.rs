#![forbid(unsafe_code)]

use std::{env, fs, path::Path};

use sha2::{Digest, Sha256};
use toml::{Table, Value};

fn main() {
    let Some(command) = env::args().nth(1) else {
        print_usage();
        std::process::exit(2);
    };

    let result = match command.as_str() {
        "corpus" => corpus(env::args().nth(2).as_deref()),
        "fixtures" => fixtures(env::args().nth(2).as_deref()),
        "feature-matrix" => feature_matrix(env::args().nth(2).as_deref()),
        _ => Err(format!("unknown xtask command: {command}")),
    };

    if let Err(message) = result {
        eprintln!("xtask: {message}");
        std::process::exit(1);
    }
}

fn print_usage() {
    eprintln!(
        "usage: cargo run -p xtask -- <corpus verify|fixtures generate-malformed|feature-matrix check>\n\
         `corpus fetch` and `corpus index` are reserved for the pinned upstream corpus workflow."
    );
}

fn fixtures(action: Option<&str>) -> Result<(), String> {
    match action {
        Some("generate-malformed") => generate_malformed_fixtures(),
        _ => Err("usage: cargo xtask fixtures generate-malformed".to_owned()),
    }
}

struct GeneratedFixture {
    id: &'static str,
    file: &'static str,
    bytes: Vec<u8>,
    feature: &'static str,
    notes: &'static str,
}

fn generate_malformed_fixtures() -> Result<(), String> {
    let valid_vp8 = riff_body(chunk(*b"VP8 ", &[0x00, 0x00], None));
    let mut trailing = valid_vp8;
    trailing.push(0xff);

    let truncated_chunk = riff_body({
        let mut body = b"WEBPVP8 ".to_vec();
        body.extend_from_slice(&1_u32.to_le_bytes());
        body
    });
    let vp8x = chunk(*b"VP8X", &[0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0], None);
    let duplicate_vp8x = riff_body([vp8x.clone(), vp8x].concat());

    let fixtures = [
        GeneratedFixture {
            id: "container-riff-declared-size-too-large-001",
            file: "riff-declared-size-too-large.webp",
            bytes: {
                let mut bytes = b"RIFF".to_vec();
                bytes.extend_from_slice(&u32::MAX.to_le_bytes());
                bytes.extend_from_slice(b"WEBP");
                bytes
            },
            feature: "riff-declared-size",
            notes: "Declared RIFF body length exceeds supplied bytes.",
        },
        GeneratedFixture {
            id: "container-chunk-payload-truncated-001",
            file: "chunk-payload-truncated.webp",
            bytes: truncated_chunk,
            feature: "chunk-payload-truncated",
            notes: "RIFF ends immediately after a one-byte VP8 payload declaration.",
        },
        GeneratedFixture {
            id: "container-riff-trailing-byte-001",
            file: "riff-trailing-byte.webp",
            bytes: trailing,
            feature: "riff-trailing-bytes",
            notes: "Strict parsing rejects data after the declared RIFF body.",
        },
        GeneratedFixture {
            id: "container-non-zero-padding-001",
            file: "non-zero-padding.webp",
            bytes: riff_body(chunk(*b"VP8 ", &[0x00], Some(0xff))),
            feature: "riff-non-zero-padding",
            notes: "Strict parsing rejects the required odd-size padding byte when non-zero.",
        },
        GeneratedFixture {
            id: "container-vp8x-reserved-bit-001",
            file: "vp8x-reserved-bit.webp",
            bytes: riff_body(chunk(*b"VP8X", &[0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0], None)),
            feature: "vp8x-reserved-bit",
            notes: "VP8X uses a reserved feature bit in the strict profile.",
        },
        GeneratedFixture {
            id: "container-duplicate-vp8x-001",
            file: "duplicate-vp8x.webp",
            bytes: duplicate_vp8x,
            feature: "duplicate-vp8x",
            notes: "Strict parsing rejects duplicate VP8X singleton chunks.",
        },
    ];
    for fixture in &fixtures {
        let fixture_path = Path::new("tests/fixtures/generated").join(fixture.file);
        write_if_changed(&fixture_path, &fixture.bytes)?;
        let digest = Sha256::digest(&fixture.bytes);
        let manifest = format!(
            "id = \"{}\"\nfile = \"../fixtures/generated/{}\"\nsha256 = \"{digest:x}\"\nclass = \"MustReject\"\nsource = \"generated: cargo xtask fixtures generate-malformed\"\nlicense = \"CC0-1.0\"\ncodec = \"Container\"\nfeatures = [\"{}\", \"public-api\", \"no-panic\"]\nnotes = \"{}\"\n",
            fixture.id, fixture.file, fixture.feature, fixture.notes
        );
        let manifest_path = Path::new("tests/manifests").join(format!("{}.toml", fixture.id));
        write_if_changed(&manifest_path, manifest.as_bytes())?;
    }
    println!("generated {} malformed container fixtures", fixtures.len());
    Ok(())
}

fn riff_body(body: Vec<u8>) -> Vec<u8> {
    let mut bytes = b"RIFF".to_vec();
    bytes.extend_from_slice(
        &u32::try_from(body.len())
            .expect("generated RIFF body length fits u32")
            .to_le_bytes(),
    );
    bytes.extend_from_slice(&body);
    bytes
}

fn chunk(fourcc: [u8; 4], payload: &[u8], padding: Option<u8>) -> Vec<u8> {
    let mut bytes = fourcc.to_vec();
    bytes.extend_from_slice(
        &u32::try_from(payload.len())
            .expect("generated chunk payload length fits u32")
            .to_le_bytes(),
    );
    bytes.extend_from_slice(payload);
    if payload.len() % 2 == 1 {
        bytes.push(padding.unwrap_or(0));
    }
    bytes
}

fn write_if_changed(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if fs::read(path).ok().as_deref() == Some(bytes) {
        return Ok(());
    }
    let parent = path
        .parent()
        .ok_or_else(|| format!("{} has no parent", path.display()))?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
    fs::write(path, bytes).map_err(|error| format!("cannot write {}: {error}", path.display()))
}

fn corpus(action: Option<&str>) -> Result<(), String> {
    match action {
        Some("verify") => verify_corpus_lock(Path::new("tools/corpus-lock.toml")),
        Some("fetch" | "index") => Err(
            "this checkout ships only the committed smoke corpus; upstream corpus fetching is not configured yet"
                .to_owned(),
        ),
        _ => Err("usage: cargo xtask corpus <verify|fetch|index>".to_owned()),
    }
}

fn feature_matrix(action: Option<&str>) -> Result<(), String> {
    if action != Some("check") {
        return Err("usage: cargo xtask feature-matrix check".to_owned());
    }
    let matrix = fs::read_to_string("tests/feature-matrix.md")
        .map_err(|error| format!("cannot read feature matrix: {error}"))?;
    for feature in ["RIFF", "VP8X", "bit reader", "checked arithmetic"] {
        if !matrix.contains(feature) {
            return Err(format!("feature matrix does not list {feature}"));
        }
    }
    println!("feature matrix: M0/M1 entries present");
    Ok(())
}

fn verify_corpus_lock(path: &Path) -> Result<(), String> {
    let lock = fs::read_to_string(path)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    validate_corpus_lock(&lock)?;
    println!("corpus lock: immutable pins verified");
    Ok(())
}

fn validate_corpus_lock(input: &str) -> Result<(), String> {
    let lock: Table = toml::from_str(input).map_err(|error| format!("invalid TOML: {error}"))?;
    if lock.get("schema_version").and_then(Value::as_integer) != Some(1) {
        return Err("corpus lock: schema_version must be 1".to_owned());
    }

    let oracle = required_table(&lock, "libwebp")?;
    require_hex(oracle, "commit", 40)?;
    require_text(oracle, "tag")?;
    require_https_url(oracle, "source_url")?;
    require_hex(oracle, "source_sha256", 64)?;
    require_text(oracle, "build_profile")?;
    require_text(oracle, "compiler")?;

    let vectors = required_table(&lock, "libwebp_test_data")?;
    require_hex(vectors, "commit", 40)?;
    require_https_url(vectors, "source_url")?;
    require_hex(vectors, "source_sha256", 64)?;
    require_text(vectors, "purpose")?;

    let clic = required_table(&lock, "clic")?;
    require_text(clic, "version")?;
    require_string_array(clic, "splits")?;
    require_text(clic, "purpose")?;
    Ok(())
}

fn required_table<'a>(lock: &'a Table, name: &str) -> Result<&'a Table, String> {
    lock.get(name)
        .and_then(Value::as_table)
        .ok_or_else(|| format!("corpus lock: missing [{name}] table"))
}

fn required_text<'a>(table: &'a Table, name: &str) -> Result<&'a str, String> {
    table
        .get(name)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("corpus lock: missing non-empty {name}"))
}

fn require_text(table: &Table, name: &str) -> Result<(), String> {
    let _ = required_text(table, name)?;
    Ok(())
}

fn require_hex(table: &Table, name: &str, length: usize) -> Result<(), String> {
    let value = required_text(table, name)?;
    if value.len() != length
        || !value.bytes().all(|byte| byte.is_ascii_hexdigit())
        || value.bytes().all(|byte| byte == b'0')
    {
        return Err(format!(
            "corpus lock: {name} must be a non-zero {length}-digit hexadecimal value"
        ));
    }
    Ok(())
}

fn require_https_url(table: &Table, name: &str) -> Result<(), String> {
    let value = required_text(table, name)?;
    if !value.starts_with("https://") {
        return Err(format!("corpus lock: {name} must use https"));
    }
    Ok(())
}

fn require_string_array(table: &Table, name: &str) -> Result<(), String> {
    let values = table
        .get(name)
        .and_then(Value::as_array)
        .filter(|values| !values.is_empty())
        .ok_or_else(|| format!("corpus lock: missing non-empty string array {name}"))?;
    if values
        .iter()
        .any(|value| value.as_str().is_none_or(|value| value.trim().is_empty()))
    {
        return Err(format!(
            "corpus lock: {name} must contain non-empty strings"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::validate_corpus_lock;

    const LOCK: &str = include_str!("../../tools/corpus-lock.toml");

    #[test]
    fn repository_lock_is_valid() {
        assert!(validate_corpus_lock(LOCK).is_ok());
    }

    #[test]
    fn rejects_zero_commit_pin() {
        let invalid = LOCK.replace(
            "commit = \"4fa21912338357f89e4fd51cf2368325b59e9bd9\"",
            "commit = \"0000000000000000000000000000000000000000\"",
        );
        assert!(validate_corpus_lock(&invalid).is_err());
    }
}
