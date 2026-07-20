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
         External corpus fetch/index commands live in tools/ and follow the configured upstream branches."
    );
}

fn fixtures(action: Option<&str>) -> Result<(), String> {
    match action {
        Some("generate-malformed") => generate_malformed_fixtures(),
        Some("generate-metadata") => generate_metadata_fixtures(),
        _ => Err("usage: cargo xtask fixtures <generate-malformed|generate-metadata>".to_owned()),
    }
}

fn generate_metadata_fixtures() -> Result<(), String> {
    const LENGTHS: [usize; 11] = [0, 1, 2, 3, 4, 7, 8, 15, 16, 255, 256];
    let mut generated = 0;
    for mask in 0_u8..8 {
        for length in LENGTHS {
            if mask == 0 && length != 0 {
                continue;
            }
            for placement in ["before", "after"] {
                let payload = (0..length)
                    .map(|index| (index as u8).wrapping_add(mask))
                    .collect::<Vec<_>>();
                let flags = (if mask & 1 != 0 { 1 << 5 } else { 0 })
                    | (if mask & 2 != 0 { 1 << 3 } else { 0 })
                    | (if mask & 4 != 0 { 1 << 2 } else { 0 });
                let mut chunks = vec![chunk(*b"VP8X", &[flags, 0, 0, 0, 0, 0, 0, 0, 0, 0], None)];
                let metadata_chunks = metadata_chunks(mask, &payload);
                if placement == "before" {
                    chunks.extend(metadata_chunks);
                    chunks.push(chunk(*b"VP8 ", &[0, 0], None));
                } else {
                    chunks.push(chunk(*b"VP8 ", &[0, 0], None));
                    chunks.extend(metadata_chunks);
                }
                let file = format!("metadata-{mask:01x}-{length:03}-{placement}.webp");
                let bytes = riff_body(chunks.concat());
                write_if_changed(&Path::new("tests/fixtures/generated").join(&file), &bytes)?;
                let mut manifest = format!(
                    "id = \"container-metadata-{mask:01x}-{length:03}-{placement}\"\nfile = \"../fixtures/generated/{file}\"\nsha256 = \"{:x}\"\nclass = \"MustAccept\"\nsource = \"generated: cargo xtask fixtures generate-metadata\"\nlicense = \"CC0-1.0\"\ncodec = \"Container\"\napi = \"ReadMetadata\"\nfeatures = [\"metadata\", \"{placement}\"]\n",
                    Sha256::digest(&bytes)
                );
                for (present, field) in [
                    (mask & 1 != 0, "iccp"),
                    (mask & 2 != 0, "exif"),
                    (mask & 4 != 0, "xmp"),
                ] {
                    if present {
                        manifest.push_str(&format!(
                            "expected_{field}_sha256 = \"{:x}\"\n",
                            Sha256::digest(&payload)
                        ));
                    }
                }
                let manifest_path = Path::new("tests/manifests").join(format!(
                    "container-metadata-{mask:01x}-{length:03}-{placement}.toml"
                ));
                write_if_changed(&manifest_path, manifest.as_bytes())?;
                generated += 1;
            }
        }
    }
    println!("generated {generated} metadata fixtures");
    Ok(())
}

fn metadata_chunks(mask: u8, payload: &[u8]) -> Vec<Vec<u8>> {
    let mut chunks = Vec::new();
    if mask & 1 != 0 {
        chunks.push(chunk(*b"ICCP", payload, None));
    }
    if mask & 2 != 0 {
        chunks.push(chunk(*b"EXIF", payload, None));
    }
    if mask & 4 != 0 {
        chunks.push(chunk(*b"XMP ", payload, None));
    }
    chunks
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
        let mut body = b"VP8 ".to_vec();
        body.extend_from_slice(&1_u32.to_le_bytes());
        body
    });
    let vp8x = chunk(*b"VP8X", &[0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0], None);
    let duplicate_vp8x = riff_body([vp8x.clone(), vp8x].concat());
    let animation_vp8x = chunk(*b"VP8X", &[0x02, 0, 0, 0, 0, 0, 0, 0, 0, 0], None);
    let truncated_anmf = riff_body({
        let mut body = animation_vp8x.clone();
        body.extend_from_slice(b"ANMF");
        body.extend_from_slice(&16_u32.to_le_bytes());
        body.extend_from_slice(&[0; 10]);
        body
    });
    let exif_vp8x = chunk(*b"VP8X", &[1 << 3, 0, 0, 0, 0, 0, 0, 0, 0, 0], None);
    let metadata_without_vp8x = riff_body(
        [
            chunk(*b"VP8 ", &[0x00, 0x00], None),
            chunk(*b"EXIF", &[0x01], None),
        ]
        .concat(),
    );

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
        GeneratedFixture {
            id: "animation-anmf-payload-truncated-001",
            file: "animation-anmf-payload-truncated.webp",
            bytes: truncated_anmf,
            feature: "animation-anmf-payload-truncated",
            notes: "ANMF declares 16 payload bytes but supplies only 10.",
        },
        GeneratedFixture {
            id: "animation-anmf-non-zero-padding-001",
            file: "animation-anmf-non-zero-padding.webp",
            bytes: riff_body([animation_vp8x, chunk(*b"ANMF", &[0x00], Some(0xff))].concat()),
            feature: "animation-anmf-non-zero-padding",
            notes: "Strict parsing rejects non-zero alignment padding after ANMF.",
        },
        GeneratedFixture {
            id: "container-duplicate-exif-001",
            file: "duplicate-exif.webp",
            bytes: riff_body(
                [
                    exif_vp8x.clone(),
                    chunk(*b"EXIF", &[0x01], None),
                    chunk(*b"EXIF", &[0x02], None),
                    chunk(*b"VP8 ", &[0x00, 0x00], None),
                ]
                .concat(),
            ),
            feature: "duplicate-metadata",
            notes: "Strict parsing rejects duplicate EXIF singleton chunks.",
        },
        GeneratedFixture {
            id: "container-metadata-without-vp8x-001",
            file: "metadata-without-vp8x.webp",
            bytes: metadata_without_vp8x,
            feature: "metadata-requires-vp8x",
            notes: "Metadata chunks require a VP8X extended header.",
        },
        GeneratedFixture {
            id: "container-vp8x-exif-flag-missing-001",
            file: "vp8x-exif-flag-missing.webp",
            bytes: riff_body(
                [
                    chunk(*b"VP8X", &[0; 10], None),
                    chunk(*b"EXIF", &[0x01], None),
                    chunk(*b"VP8 ", &[0x00, 0x00], None),
                ]
                .concat(),
            ),
            feature: "vp8x-metadata-flag",
            notes: "EXIF is present but its VP8X feature flag is clear.",
        },
        GeneratedFixture {
            id: "container-vp8x-exif-flag-without-chunk-001",
            file: "vp8x-exif-flag-without-chunk.webp",
            bytes: riff_body([exif_vp8x.clone(), chunk(*b"VP8 ", &[0x00, 0x00], None)].concat()),
            feature: "vp8x-metadata-flag",
            notes: "The VP8X EXIF feature flag is set but no EXIF chunk is present.",
        },
        GeneratedFixture {
            id: "container-vp8x-not-first-001",
            file: "vp8x-not-first.webp",
            bytes: riff_body([chunk(*b"VP8 ", &[0x00, 0x00], None), exif_vp8x].concat()),
            feature: "vp8x-layout",
            notes: "An extended header must be the first chunk in a strict container.",
        },
        GeneratedFixture {
            id: "container-mixed-vp8-vp8l-001",
            file: "mixed-vp8-vp8l.webp",
            bytes: riff_body(
                [
                    chunk(*b"VP8 ", &[0x00, 0x00], None),
                    chunk(*b"VP8L", &[0x2f, 0x00, 0x00, 0x00, 0x00], None),
                ]
                .concat(),
            ),
            feature: "mixed-image-codecs",
            notes: "A container cannot carry both a VP8 and a VP8L image chunk.",
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
    let mut body_with_form_type = b"WEBP".to_vec();
    body_with_form_type.extend_from_slice(&body);
    let mut bytes = b"RIFF".to_vec();
    bytes.extend_from_slice(
        &u32::try_from(body_with_form_type.len())
            .expect("generated RIFF body length fits u32")
            .to_le_bytes(),
    );
    bytes.extend_from_slice(&body_with_form_type);
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
    println!("corpus sources: pinned configuration verified");
    Ok(())
}

fn validate_corpus_lock(input: &str) -> Result<(), String> {
    let lock: Table = toml::from_str(input).map_err(|error| format!("invalid TOML: {error}"))?;
    let oracle = required_table(&lock, "libwebp")?;
    require_https_url(oracle, "source_url")?;
    require_text(oracle, "commit")?;
    require_text(oracle, "tracking_branch")?;
    require_text(oracle, "build_profile")?;
    require_text(oracle, "compiler")?;

    let vectors = required_table(&lock, "libwebp_test_data")?;
    require_https_url(vectors, "source_url")?;
    require_text(vectors, "commit")?;
    require_text(vectors, "tracking_branch")?;
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
    fn rejects_missing_tracking_branch() {
        let invalid = LOCK.replace("tracking_branch = \"main\"", "tracking_branch = \"\"");
        assert!(validate_corpus_lock(&invalid).is_err());
    }
}
