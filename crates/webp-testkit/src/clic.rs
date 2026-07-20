//! Integrity checking for the ignored CLIC PNG benchmark corpus.

use std::{
    fmt, fs,
    path::{Path, PathBuf},
};

use serde::Deserialize;

use crate::sha256_hex;

/// Parsed CLIC validation manifest produced by `fetch-clic-validation.py`.
#[derive(Debug, Deserialize)]
pub struct ClicManifest {
    pub dataset: String,
    pub split: String,
    pub images: Vec<ClicImage>,
}

/// Integrity and geometry record for one benchmark PNG.
#[derive(Debug, Deserialize)]
pub struct ClicImage {
    pub id: String,
    pub file: PathBuf,
    pub sha256: String,
    pub width: u32,
    pub height: u32,
    pub channels: u8,
}

/// Totals verified from a CLIC validation manifest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClicSummary {
    pub images: usize,
    pub bytes: usize,
}

/// Error while loading or verifying CLIC benchmark inputs.
#[derive(Debug)]
pub enum ClicError {
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    Json(serde_json::Error),
    Invalid(&'static str),
    PathEscape {
        path: PathBuf,
        root: PathBuf,
    },
    HashMismatch {
        path: PathBuf,
        expected: String,
        actual: String,
    },
}

impl fmt::Display for ClicError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read { path, source } => write!(f, "cannot read {}: {source}", path.display()),
            Self::Json(error) => write!(f, "invalid CLIC manifest JSON: {error}"),
            Self::Invalid(message) => write!(f, "invalid CLIC manifest: {message}"),
            Self::PathEscape { path, root } => write!(
                f,
                "CLIC image {} escapes {}",
                path.display(),
                root.display()
            ),
            Self::HashMismatch {
                path,
                expected,
                actual,
            } => write!(
                f,
                "CLIC checksum mismatch for {}: expected {expected}, got {actual}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for ClicError {}

/// Verifies every image selected by a local CLIC validation manifest.
pub fn verify_clic_validation(root: impl AsRef<Path>) -> Result<ClicSummary, ClicError> {
    let root = root.as_ref();
    let manifest_path = root.join("validation-manifest.json");
    let text = fs::read_to_string(&manifest_path).map_err(|source| ClicError::Read {
        path: manifest_path,
        source,
    })?;
    let manifest: ClicManifest = serde_json::from_str(&text).map_err(ClicError::Json)?;
    if manifest.dataset != "tfds:clic:1.0.0"
        || manifest.split != "validation"
        || manifest.images.is_empty()
    {
        return Err(ClicError::Invalid("dataset, split, or image list"));
    }
    let image_root =
        fs::canonicalize(root.join("validation-png")).map_err(|source| ClicError::Read {
            path: root.join("validation-png"),
            source,
        })?;
    let mut summary = ClicSummary {
        images: 0,
        bytes: 0,
    };
    for image in manifest.images {
        if image.id.trim().is_empty()
            || image.file.is_absolute()
            || image.width == 0
            || image.height == 0
            || !(1..=4).contains(&image.channels)
        {
            return Err(ClicError::Invalid("image id, path, geometry, or channels"));
        }
        if image.sha256.len() != 64 || !image.sha256.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(ClicError::Invalid("image sha256"));
        }
        let requested = image_root.join(&image.file);
        let path = fs::canonicalize(&requested).map_err(|source| ClicError::Read {
            path: requested,
            source,
        })?;
        if !path.starts_with(&image_root) {
            return Err(ClicError::PathEscape {
                path,
                root: image_root.clone(),
            });
        }
        let bytes = fs::read(&path).map_err(|source| ClicError::Read {
            path: path.clone(),
            source,
        })?;
        let actual = sha256_hex(&bytes);
        if actual != image.sha256.to_ascii_lowercase() {
            return Err(ClicError::HashMismatch {
                path,
                expected: image.sha256,
                actual,
            });
        }
        summary.images += 1;
        summary.bytes += bytes.len();
    }
    Ok(summary)
}
