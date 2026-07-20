//! Fixture manifest parsing and execution helpers.

use std::{
    fmt, fs,
    path::{Component, Path, PathBuf},
};

use serde::Deserialize;
use sha2::{Digest, Sha256};

/// Classification of the expected outcome for a fixture.
///
/// Invalid inputs are deliberately not treated as one class: compatibility
/// tests must not accidentally turn a permissive oracle behaviour into a
/// permanent requirement for strict parsing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum FixtureClass {
    #[serde(rename = "MustAccept")]
    MustAccept,
    #[serde(rename = "MustReject")]
    MustReject,
    #[serde(rename = "CompatAccept")]
    CompatAccept,
    #[serde(rename = "ImplementationDefined")]
    ImplementationDefined,
}

/// Codec carried by an image or animation fixture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum Codec {
    #[serde(rename = "VP8")]
    Vp8,
    #[serde(rename = "VP8L")]
    Vp8l,
    #[serde(rename = "Mixed")]
    Mixed,
    #[serde(rename = "Container")]
    Container,
}

/// One TOML manifest sidecar for a WebP fixture.
///
/// `source` is deliberately free-form.  It normally names `generated`, an
/// upstream corpus plus its pinned revision, or a public bug report.  `license`
/// is required so a fixture can safely move between the smoke and regression
/// corpus.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct FixtureManifest {
    pub id: String,
    pub file: PathBuf,
    pub sha256: String,
    pub class: FixtureClass,
    pub source: String,
    pub license: String,
    pub codec: Codec,
    #[serde(default)]
    pub features: Vec<String>,
    #[serde(default)]
    pub expected_width: Option<u32>,
    #[serde(default)]
    pub expected_height: Option<u32>,
    #[serde(default)]
    pub expected_rgba_sha256: Option<String>,
    #[serde(default)]
    pub max_work_units: Option<u64>,
    #[serde(default)]
    pub max_alloc_bytes: Option<usize>,
    #[serde(default)]
    pub notes: Option<String>,
}

impl FixtureManifest {
    /// Checks data-independent manifest invariants.
    pub fn validate(&self) -> Result<(), ManifestError> {
        if self.id.trim().is_empty() {
            return Err(ManifestError::InvalidField("id must not be empty"));
        }
        if self.source.trim().is_empty() {
            return Err(ManifestError::InvalidField("source must not be empty"));
        }
        if self.license.trim().is_empty() {
            return Err(ManifestError::InvalidField("license must not be empty"));
        }
        validate_relative_file(&self.file)?;
        validate_sha256("sha256", &self.sha256)?;
        if let Some(hash) = &self.expected_rgba_sha256 {
            validate_sha256("expected_rgba_sha256", hash)?;
        }
        if self.expected_width.is_some() != self.expected_height.is_some() {
            return Err(ManifestError::InvalidField(
                "expected_width and expected_height must be specified together",
            ));
        }
        if matches!(
            self.class,
            FixtureClass::MustAccept | FixtureClass::CompatAccept
        ) && self.expected_width.zip(self.expected_height).is_none()
        {
            return Err(ManifestError::InvalidField(
                "accepted fixtures require expected_width and expected_height",
            ));
        }
        Ok(())
    }
}

/// Parses and validates one fixture manifest TOML document.
pub fn parse_manifest(input: &str) -> Result<FixtureManifest, ManifestError> {
    let manifest: FixtureManifest = toml::from_str(input).map_err(ManifestError::Toml)?;
    manifest.validate()?;
    Ok(manifest)
}

/// Failure while parsing a manifest or checking its local invariants.
#[derive(Debug)]
pub enum ManifestError {
    Toml(toml::de::Error),
    InvalidField(&'static str),
}

impl fmt::Display for ManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Toml(error) => write!(f, "invalid fixture manifest TOML: {error}"),
            Self::InvalidField(message) => write!(f, "invalid fixture manifest: {message}"),
        }
    }
}

impl std::error::Error for ManifestError {}

/// Reusable runner for a corpus root containing `*.toml` manifest sidecars.
///
/// The callback receives the parsed manifest and original WebP bytes.  It owns
/// the assertion policy for the public decoder API while this runner owns
/// discovery, parsing, path containment, and byte-level integrity checks.
#[derive(Debug, Clone)]
pub struct FixtureRunner {
    root: PathBuf,
}

impl FixtureRunner {
    /// Creates a runner rooted at a fixture directory.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Runs every `*.toml` sidecar below the corpus root in deterministic path
    /// order.  Fixture paths in manifests are relative to their sidecar; they
    /// may use `..` to refer from `tests/manifests` to `tests/fixtures`, but
    /// the resolved file must remain under this runner's corpus root.
    pub fn run_all<E>(
        &self,
        mut check: impl FnMut(&FixtureManifest, &[u8]) -> Result<(), E>,
    ) -> Result<RunSummary, RunError<E>> {
        let mut manifests = self.manifest_paths()?;
        manifests.sort();

        let mut summary = RunSummary::default();
        for manifest_path in manifests {
            let text = fs::read_to_string(&manifest_path).map_err(|source| RunError::Read {
                path: manifest_path.clone(),
                source,
            })?;
            let manifest = parse_manifest(&text).map_err(|source| RunError::Manifest {
                path: manifest_path.clone(),
                source,
            })?;
            let fixture_path = self.fixture_path(&manifest_path, &manifest.file)?;
            let bytes = fs::read(&fixture_path).map_err(|source| RunError::Read {
                path: fixture_path.clone(),
                source,
            })?;
            verify_sha256(&fixture_path, &manifest.sha256, &bytes)?;
            check(&manifest, &bytes).map_err(|source| RunError::Check {
                id: manifest.id.clone(),
                source,
            })?;
            summary.fixtures += 1;
            summary.bytes += bytes.len();
        }
        Ok(summary)
    }

    fn manifest_paths<E>(&self) -> Result<Vec<PathBuf>, RunError<E>> {
        let mut paths = Vec::new();
        self.collect_manifest_paths(&self.root, &mut paths)?;
        Ok(paths)
    }

    fn collect_manifest_paths<E>(
        &self,
        directory: &Path,
        paths: &mut Vec<PathBuf>,
    ) -> Result<(), RunError<E>> {
        let entries = fs::read_dir(directory).map_err(|source| RunError::Read {
            path: directory.to_path_buf(),
            source,
        })?;
        for entry in entries {
            let entry = entry.map_err(|source| RunError::Read {
                path: directory.to_path_buf(),
                source,
            })?;
            let path = entry.path();
            let file_type = entry.file_type().map_err(|source| RunError::Read {
                path: path.clone(),
                source,
            })?;
            if file_type.is_dir() {
                self.collect_manifest_paths(&path, paths)?;
            } else if file_type.is_file()
                && path
                    .extension()
                    .is_some_and(|extension| extension == "toml")
            {
                paths.push(path);
            }
        }
        Ok(())
    }

    fn fixture_path<E>(
        &self,
        manifest_path: &Path,
        fixture: &Path,
    ) -> Result<PathBuf, RunError<E>> {
        let parent = manifest_path.parent().unwrap_or(&self.root);
        let requested = parent.join(fixture);
        let resolved = fs::canonicalize(&requested).map_err(|source| RunError::Read {
            path: requested,
            source,
        })?;
        let root = fs::canonicalize(&self.root).map_err(|source| RunError::Read {
            path: self.root.clone(),
            source,
        })?;
        if resolved.starts_with(&root) {
            Ok(resolved)
        } else {
            Err(RunError::PathEscape {
                path: resolved,
                root,
            })
        }
    }
}

/// Aggregate information produced by [`FixtureRunner::run_all`].
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct RunSummary {
    pub fixtures: usize,
    pub bytes: usize,
}

/// Failure while discovering, reading, verifying, or checking fixtures.
#[derive(Debug)]
pub enum RunError<E> {
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    Manifest {
        path: PathBuf,
        source: ManifestError,
    },
    HashMismatch {
        path: PathBuf,
        expected: String,
        actual: String,
    },
    PathEscape {
        path: PathBuf,
        root: PathBuf,
    },
    Check {
        id: String,
        source: E,
    },
}

impl<E: fmt::Display> fmt::Display for RunError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read { path, source } => write!(f, "cannot read {}: {source}", path.display()),
            Self::Manifest { path, source } => {
                write!(f, "invalid manifest {}: {source}", path.display())
            }
            Self::HashMismatch {
                path,
                expected,
                actual,
            } => write!(
                f,
                "fixture checksum mismatch for {}: expected {expected}, got {actual}",
                path.display()
            ),
            Self::PathEscape { path, root } => write!(
                f,
                "fixture path {} escapes corpus root {}",
                path.display(),
                root.display()
            ),
            Self::Check { id, source } => write!(f, "fixture {id} failed: {source}"),
        }
    }
}

impl<E: fmt::Debug + fmt::Display> std::error::Error for RunError<E> {}

fn validate_relative_file(path: &Path) -> Result<(), ManifestError> {
    if path.as_os_str().is_empty() || path.is_absolute() {
        return Err(ManifestError::InvalidField(
            "file must be a non-empty relative path",
        ));
    }
    if path
        .components()
        .any(|component| matches!(component, Component::RootDir | Component::Prefix(_)))
    {
        return Err(ManifestError::InvalidField(
            "file must be relative to the manifest",
        ));
    }
    Ok(())
}

fn validate_sha256(field: &'static str, value: &str) -> Result<(), ManifestError> {
    if value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Ok(())
    } else if field == "sha256" {
        Err(ManifestError::InvalidField(
            "sha256 must contain exactly 64 hexadecimal characters",
        ))
    } else {
        Err(ManifestError::InvalidField(
            "expected_rgba_sha256 must contain exactly 64 hexadecimal characters",
        ))
    }
}

fn verify_sha256<E>(path: &Path, expected: &str, bytes: &[u8]) -> Result<(), RunError<E>> {
    let actual = hex_sha256(bytes);
    if actual == expected.to_ascii_lowercase() {
        Ok(())
    } else {
        Err(RunError::HashMismatch {
            path: path.to_path_buf(),
            expected: expected.to_owned(),
            actual,
        })
    }
}

/// Returns the lower-case hexadecimal SHA-256 digest of `bytes`.
pub fn hex_sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    format!("{digest:x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    const HASH: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

    fn manifest_with(class: &str, file: &str, hash: &str, fields: &str) -> String {
        format!(
            "id = \"empty\"\nfile = \"{file}\"\nsha256 = \"{hash}\"\nclass = \"{class}\"\nsource = \"generated\"\nlicense = \"CC0-1.0\"\ncodec = \"Container\"\n{fields}"
        )
    }

    fn manifest(fields: &str) -> String {
        manifest_with("MustReject", "empty.webp", HASH, fields)
    }

    #[test]
    fn parses_minimal_rejection_manifest() {
        let parsed = parse_manifest(&manifest("")).expect("manifest should parse");
        assert_eq!(parsed.class, FixtureClass::MustReject);
        assert_eq!(parsed.file, PathBuf::from("empty.webp"));
    }

    #[test]
    fn accepts_require_dimensions() {
        let error = parse_manifest(&manifest_with("MustAccept", "empty.webp", HASH, ""))
            .expect_err("accepted manifest without dimensions must fail");
        assert!(error.to_string().contains("expected_width"));
    }

    #[test]
    fn permits_a_sibling_fixture_directory() {
        let parsed = parse_manifest(&manifest_with(
            "MustReject",
            "../fixtures/empty.webp",
            HASH,
            "",
        ))
        .expect("a sibling fixture directory is valid");
        assert_eq!(parsed.file, PathBuf::from("../fixtures/empty.webp"));
    }

    #[test]
    fn rejects_invalid_hash() {
        let error = parse_manifest(&manifest_with("MustReject", "empty.webp", "not-a-hash", ""))
            .expect_err("short hash must fail");
        assert!(error.to_string().contains("sha256"));
    }

    #[test]
    fn runner_discovers_sorts_and_checks_files() {
        let root = std::env::temp_dir().join(format!("webp-testkit-runner-{}", std::process::id()));
        fs::create_dir_all(&root).expect("temporary fixture directory");
        fs::create_dir_all(root.join("manifests")).expect("manifest directory");
        fs::create_dir_all(root.join("fixtures")).expect("fixture directory");
        fs::write(root.join("fixtures/z.webp"), b"z").expect("fixture file");
        fs::write(root.join("fixtures/a.webp"), b"a").expect("fixture file");
        fs::write(
            root.join("manifests/z.toml"),
            "id = \"z\"\nfile = \"../fixtures/z.webp\"\nsha256 = \"594e519ae499312b29433b7dd8a97ff068defcba9755b6d5d00e84c524d67b06\"\nclass = \"MustReject\"\nsource = \"generated\"\nlicense = \"CC0-1.0\"\ncodec = \"Container\"\n",
        )
        .expect("manifest");
        fs::write(
            root.join("manifests/a.toml"),
            "id = \"a\"\nfile = \"../fixtures/a.webp\"\nsha256 = \"ca978112ca1bbdcafac231b39a23dc4da786eff8147c4e72b9807785afee48bb\"\nclass = \"MustReject\"\nsource = \"generated\"\nlicense = \"CC0-1.0\"\ncodec = \"Container\"\n",
        )
        .expect("manifest");

        let mut ids = Vec::new();
        let summary = FixtureRunner::new(&root)
            .run_all(|fixture, _| {
                ids.push(fixture.id.clone());
                Ok::<_, String>(())
            })
            .expect("runner should succeed");
        assert_eq!(ids, ["a", "z"]);
        assert_eq!(
            summary,
            RunSummary {
                fixtures: 2,
                bytes: 2
            }
        );
        fs::remove_dir_all(root).expect("temporary fixture cleanup");
    }

    #[test]
    fn runner_reports_checksum_mismatch_before_callback() {
        let root = std::env::temp_dir().join(format!("webp-testkit-hash-{}", std::process::id()));
        fs::create_dir_all(&root).expect("temporary fixture directory");
        fs::create_dir_all(root.join("manifests")).expect("manifest directory");
        fs::create_dir_all(root.join("fixtures")).expect("fixture directory");
        fs::write(root.join("fixtures/bad.webp"), b"unexpected").expect("fixture file");
        fs::write(
            root.join("manifests/bad.toml"),
            manifest_with("MustReject", "../fixtures/bad.webp", HASH, ""),
        )
        .expect("manifest");
        let result = FixtureRunner::new(&root).run_all(|_, _| Ok::<_, String>(()));
        assert!(matches!(result, Err(RunError::HashMismatch { .. })));
        fs::remove_dir_all(root).expect("temporary fixture cleanup");
    }

    #[test]
    fn runner_rejects_paths_outside_the_corpus_root() {
        let parent =
            std::env::temp_dir().join(format!("webp-testkit-escape-{}", std::process::id()));
        let root = parent.join("corpus");
        fs::create_dir_all(root.join("manifests")).expect("manifest directory");
        fs::write(parent.join("outside.webp"), b"outside").expect("outside fixture");
        fs::write(
            root.join("manifests/escape.toml"),
            manifest_with(
                "MustReject",
                "../../outside.webp",
                "31207a2065f46a5b948fce6fe5c13e85abaf5631e2f894b47dcd4fce14f6c57b",
                "",
            ),
        )
        .expect("manifest");

        let result = FixtureRunner::new(&root).run_all(|_, _| Ok::<_, String>(()));
        assert!(matches!(result, Err(RunError::PathEscape { .. })));
        fs::remove_dir_all(parent).expect("temporary fixture cleanup");
    }
}
