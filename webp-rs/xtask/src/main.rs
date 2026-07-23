#![forbid(unsafe_code)]

use std::env;
use std::fs;
use std::path::Path;

use toml::Table;
use toml::Value;

mod fixture_cache;
mod fixture_set;
mod sha256;

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

fn repository_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("xtask resides in the webp-rs workspace")
}

fn print_usage() {
    eprintln!(
        "usage: cargo run -p xtask -- <corpus verify|fixtures ensure|fixtures verify|feature-matrix check>\n\
         External corpus fetch/index commands live in tools/ and follow the configured upstream branches."
    );
}

fn fixtures(action: Option<&str>) -> Result<(), String> {
    let root = repository_root().join("tests/fixtures/generated");
    let generated = fixture_set::generate();
    let summary = match action {
        Some("ensure" | "generate") => fixture_cache::ensure(&root, &generated)?,
        Some("verify") => fixture_cache::verify(&root, &generated)?,
        _ => {
            return Err("usage: cargo xtask fixtures <ensure|verify>".to_owned());
        }
    };
    if action == Some("generate") {
        eprintln!("xtask: `fixtures generate` is deprecated; use `fixtures ensure`");
    }
    println!(
        "fixture cache: {:?}, {} files, identity {}",
        summary.outcome, summary.count, summary.digest
    );
    Ok(())
}

fn corpus(action: Option<&str>) -> Result<(), String> {
    match action {
        Some("verify") => verify_corpus_lock(&repository_root().join("tools/corpus-lock.toml")),
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
    let matrix = fs::read_to_string(repository_root().join("tests/feature-matrix.md"))
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

    const LOCK: &str = include_str!("../../../tools/corpus-lock.toml");

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
