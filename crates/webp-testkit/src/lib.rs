//! Shared test-only infrastructure for the WebP implementation.
//!
//! Fixture manifests are intentionally data-driven: a test chooses the public
//! API to exercise from the manifest instead of encoding expectations in an
//! ad-hoc list of filenames.  This crate has no dependency on a codec crate so
//! it can also be used by fuzz-regression and oracle test binaries.

#![forbid(unsafe_code)]

pub mod fixture;

pub use fixture::{
    parse_manifest, Codec, FixtureClass, FixtureManifest, FixtureRunner, ManifestError, RunError,
    RunSummary,
};
