#![forbid(unsafe_code)]

use std::{env, fs, path::Path};

fn main() {
    let Some(command) = env::args().nth(1) else {
        print_usage();
        std::process::exit(2);
    };

    let result = match command.as_str() {
        "corpus" => corpus(env::args().nth(2).as_deref()),
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
        "usage: cargo run -p xtask -- <corpus verify|feature-matrix check>\n\
         `corpus fetch` and `corpus index` are reserved for the pinned upstream corpus workflow."
    );
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
    for field in ["commit", "source_sha256", "build_profile", "compiler"] {
        if !lock.contains(field) {
            return Err(format!(
                "{} is missing required field {field}",
                path.display()
            ));
        }
    }
    println!(
        "corpus lock: schema verified (network fetch intentionally disabled during M1 groundwork)"
    );
    Ok(())
}
