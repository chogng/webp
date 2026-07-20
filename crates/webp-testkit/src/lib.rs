//! Shared test-only infrastructure for the WebP implementation.
//!
//! Fixture manifests are intentionally data-driven: a test chooses the public
//! API to exercise from the manifest instead of encoding expectations in an
//! ad-hoc list of filenames.  This crate has no dependency on a codec crate so
//! it can also be used by fuzz-regression and oracle test binaries.

#![forbid(unsafe_code)]

pub mod clic;
pub mod fixture;

pub use clic::{verify_clic_validation, ClicError, ClicImage, ClicManifest, ClicSummary};
pub use fixture::{
    parse_manifest, sha256_hex, Codec, FixtureApi, FixtureClass, FixtureManifest, FixtureRunner,
    ManifestError, RunError, RunSummary,
};
