use std::collections::BTreeSet;
use std::fs;
use std::fs::OpenOptions;
use std::io::ErrorKind;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::thread;
use std::time::Duration;
use std::time::Instant;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use crate::fixture_set::Fixture;
use crate::sha256::hex_digest;

const MANIFEST_HEADER: &str = "webp-fixture-manifest-v1";
const MANIFEST_NAME: &str = "MANIFEST.sha256";
const SETS_DIRECTORY: &str = "sets";
const CURRENT_PREFIX: &str = "CURRENT-";
const LOCK_FILE: &str = ".generated.lock";
const LOCK_TIMEOUT: Duration = Duration::from_secs(30);
const STALE_LOCK_AGE: Duration = Duration::from_secs(300);

static UNIQUE_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EnsureOutcome {
    CacheHit,
    Published,
}

#[derive(Debug)]
pub(crate) struct CacheSummary {
    pub(crate) count: usize,
    pub(crate) digest: String,
    pub(crate) outcome: EnsureOutcome,
}

struct ExpectedManifest {
    bytes: Vec<u8>,
    digest: String,
}

struct CurrentMarker {
    sequence: u64,
    digest: String,
    path: PathBuf,
}

struct CacheLock {
    path: PathBuf,
}

impl Drop for CacheLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

struct StagingDirectory {
    path: PathBuf,
    published: bool,
}

impl Drop for StagingDirectory {
    fn drop(&mut self) {
        if !self.published {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

pub(crate) fn ensure(root: &Path, fixtures: &[Fixture]) -> Result<CacheSummary, String> {
    let expected = expected_manifest(fixtures)?;
    if verify_expected(root, fixtures, &expected).is_ok() {
        return Ok(CacheSummary {
            count: fixtures.len(),
            digest: expected.digest,
            outcome: EnsureOutcome::CacheHit,
        });
    }

    let _lock = acquire_lock(root)?;
    if verify_expected(root, fixtures, &expected).is_ok() {
        return Ok(CacheSummary {
            count: fixtures.len(),
            digest: expected.digest,
            outcome: EnsureOutcome::CacheHit,
        });
    }

    publish(root, fixtures, &expected)?;
    verify_expected(root, fixtures, &expected)?;
    Ok(CacheSummary {
        count: fixtures.len(),
        digest: expected.digest,
        outcome: EnsureOutcome::Published,
    })
}

pub(crate) fn verify(root: &Path, fixtures: &[Fixture]) -> Result<CacheSummary, String> {
    let expected = expected_manifest(fixtures)?;
    verify_expected(root, fixtures, &expected)?;
    Ok(CacheSummary {
        count: fixtures.len(),
        digest: expected.digest,
        outcome: EnsureOutcome::CacheHit,
    })
}

fn expected_manifest(fixtures: &[Fixture]) -> Result<ExpectedManifest, String> {
    let mut names = BTreeSet::new();
    let mut bytes = format!("{MANIFEST_HEADER}\n").into_bytes();
    for fixture in fixtures {
        validate_name(&fixture.name)?;
        if !names.insert(&fixture.name) {
            return Err(format!("duplicate generated fixture {}", fixture.name));
        }
        writeln!(
            bytes,
            "{} {} {}",
            hex_digest(&fixture.bytes),
            fixture.bytes.len(),
            fixture.name
        )
        .expect("writing a manifest to Vec cannot fail");
    }
    Ok(ExpectedManifest {
        digest: hex_digest(&bytes),
        bytes,
    })
}

fn validate_name(name: &str) -> Result<(), String> {
    if name.is_empty()
        || !name.ends_with(".webp")
        || name.contains(['/', '\\', '\n', '\r', ' '])
        || name == "."
        || name == ".."
    {
        return Err(format!("unsafe generated fixture name {name:?}"));
    }
    Ok(())
}

fn verify_expected(
    root: &Path,
    fixtures: &[Fixture],
    expected: &ExpectedManifest,
) -> Result<(), String> {
    let marker = current_marker(root)?;
    if marker.digest != expected.digest {
        return Err(format!(
            "fixture cache identity mismatch: current {}, expected {}",
            marker.digest, expected.digest
        ));
    }
    let marker_contents = fs::read_to_string(&marker.path)
        .map_err(|error| format!("cannot read {}: {error}", marker.path.display()))?;
    if marker_contents != format!("{}\n", marker.digest) {
        return Err(format!(
            "fixture cache marker {} is malformed",
            marker.path.display()
        ));
    }
    verify_generation(
        &root.join(SETS_DIRECTORY).join(&marker.digest),
        fixtures,
        expected,
    )
}

fn verify_generation(
    directory: &Path,
    fixtures: &[Fixture],
    expected: &ExpectedManifest,
) -> Result<(), String> {
    let manifest_path = directory.join(MANIFEST_NAME);
    let manifest = fs::read(&manifest_path)
        .map_err(|error| format!("cannot read {}: {error}", manifest_path.display()))?;
    if manifest != expected.bytes {
        return Err(format!(
            "fixture manifest {} does not match the generator",
            manifest_path.display()
        ));
    }

    let mut actual_names = BTreeSet::new();
    for entry in fs::read_dir(directory)
        .map_err(|error| format!("cannot read {}: {error}", directory.display()))?
    {
        let entry =
            entry.map_err(|error| format!("cannot read {} entry: {error}", directory.display()))?;
        if !entry
            .file_type()
            .map_err(|error| format!("cannot inspect {}: {error}", entry.path().display()))?
            .is_file()
        {
            return Err(format!(
                "fixture generation contains non-file {}",
                entry.path().display()
            ));
        }
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| "fixture generation contains a non-UTF-8 file name".to_owned())?;
        actual_names.insert(name);
    }

    let expected_names = fixtures
        .iter()
        .map(|fixture| fixture.name.clone())
        .chain(std::iter::once(MANIFEST_NAME.to_owned()))
        .collect::<BTreeSet<_>>();
    if actual_names != expected_names {
        return Err(format!(
            "fixture generation {} has an incomplete or unexpected file set",
            directory.display()
        ));
    }

    for fixture in fixtures {
        let path = directory.join(&fixture.name);
        let actual =
            fs::read(&path).map_err(|error| format!("cannot read {}: {error}", path.display()))?;
        if actual != fixture.bytes {
            return Err(format!(
                "fixture {} failed integrity validation",
                path.display()
            ));
        }
    }
    Ok(())
}

fn publish(root: &Path, fixtures: &[Fixture], expected: &ExpectedManifest) -> Result<(), String> {
    fs::create_dir_all(root)
        .map_err(|error| format!("cannot create {}: {error}", root.display()))?;
    let sets = root.join(SETS_DIRECTORY);
    fs::create_dir_all(&sets)
        .map_err(|error| format!("cannot create {}: {error}", sets.display()))?;
    let final_directory = sets.join(&expected.digest);

    if final_directory.exists() && verify_generation(&final_directory, fixtures, expected).is_err()
    {
        let quarantine = sets.join(format!(".corrupt-{}-{}", expected.digest, unique_suffix()));
        fs::rename(&final_directory, &quarantine).map_err(|error| {
            format!(
                "cannot quarantine corrupt fixture generation {}: {error}",
                final_directory.display()
            )
        })?;
    }

    if !final_directory.exists() {
        let staging_path = sets.join(format!(".staging-{}", unique_suffix()));
        fs::create_dir(&staging_path)
            .map_err(|error| format!("cannot create {}: {error}", staging_path.display()))?;
        let mut staging = StagingDirectory {
            path: staging_path,
            published: false,
        };
        for fixture in fixtures {
            write_synced(&staging.path.join(&fixture.name), &fixture.bytes)?;
        }
        write_synced(&staging.path.join(MANIFEST_NAME), &expected.bytes)?;
        fs::rename(&staging.path, &final_directory).map_err(|error| {
            format!(
                "cannot publish fixture generation {}: {error}",
                final_directory.display()
            )
        })?;
        staging.published = true;
    }

    let markers = current_markers(root)?;
    let sequence = markers
        .iter()
        .map(|marker| marker.sequence)
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or_else(|| "fixture cache marker sequence overflowed".to_owned())?;
    let marker_name = format!("{CURRENT_PREFIX}{sequence:020}-{}", expected.digest);
    let staging_marker = root.join(format!(".marker-{}", unique_suffix()));
    write_synced(&staging_marker, format!("{}\n", expected.digest).as_bytes())?;
    let marker_path = root.join(marker_name);
    fs::rename(&staging_marker, &marker_path)
        .map_err(|error| format!("cannot publish {}: {error}", marker_path.display()))?;

    for marker in markers {
        if marker.path != marker_path {
            fs::remove_file(&marker.path)
                .map_err(|error| format!("cannot retire {}: {error}", marker.path.display()))?;
        }
    }
    Ok(())
}

fn write_synced(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| format!("cannot create {}: {error}", path.display()))?;
    file.write_all(bytes)
        .map_err(|error| format!("cannot write {}: {error}", path.display()))?;
    file.sync_all()
        .map_err(|error| format!("cannot sync {}: {error}", path.display()))
}

fn current_marker(root: &Path) -> Result<CurrentMarker, String> {
    current_markers(root)?
        .into_iter()
        .max_by_key(|marker| marker.sequence)
        .ok_or_else(|| {
            "fixture cache is absent; run `cargo run -p xtask -- fixtures ensure`".to_owned()
        })
}

fn current_markers(root: &Path) -> Result<Vec<CurrentMarker>, String> {
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(format!("cannot read {}: {error}", root.display())),
    };
    let mut markers = Vec::new();
    for entry in entries {
        let entry =
            entry.map_err(|error| format!("cannot read {} entry: {error}", root.display()))?;
        let name = match entry.file_name().into_string() {
            Ok(name) => name,
            Err(_) => continue,
        };
        let Some(rest) = name.strip_prefix(CURRENT_PREFIX) else {
            continue;
        };
        let Some((sequence, digest)) = rest.split_once('-') else {
            continue;
        };
        if sequence.len() != 20
            || digest.len() != 64
            || !sequence.bytes().all(|byte| byte.is_ascii_digit())
            || !digest.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            continue;
        }
        if !entry
            .file_type()
            .map_err(|error| format!("cannot inspect {}: {error}", entry.path().display()))?
            .is_file()
        {
            continue;
        }
        markers.push(CurrentMarker {
            sequence: sequence
                .parse()
                .map_err(|error| format!("invalid fixture marker {name}: {error}"))?,
            digest: digest.to_ascii_lowercase(),
            path: entry.path(),
        });
    }
    Ok(markers)
}

fn acquire_lock(root: &Path) -> Result<CacheLock, String> {
    let parent = root
        .parent()
        .ok_or_else(|| format!("{} has no parent", root.display()))?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
    let path = parent.join(LOCK_FILE);
    let started = Instant::now();
    loop {
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut file) => {
                writeln!(
                    file,
                    "pid={} created_unix_seconds={}",
                    std::process::id(),
                    unix_seconds()
                )
                .map_err(|error| format!("cannot write {}: {error}", path.display()))?;
                file.sync_all()
                    .map_err(|error| format!("cannot sync {}: {error}", path.display()))?;
                return Ok(CacheLock { path });
            }
            Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                if lock_is_stale(&path) {
                    let stale = parent.join(format!(".generated.stale-{}", unique_suffix()));
                    match fs::rename(&path, &stale) {
                        Ok(()) => {
                            let _ = fs::remove_file(stale);
                            continue;
                        }
                        Err(rename_error) if rename_error.kind() == ErrorKind::NotFound => continue,
                        Err(_) => {}
                    }
                }
                if started.elapsed() >= LOCK_TIMEOUT {
                    return Err(format!(
                        "timed out waiting for fixture cache lock {}",
                        path.display()
                    ));
                }
                thread::sleep(Duration::from_millis(25));
            }
            Err(error) => return Err(format!("cannot acquire {}: {error}", path.display())),
        }
    }
}

fn lock_is_stale(path: &Path) -> bool {
    fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .ok()
        .and_then(|modified| modified.elapsed().ok())
        .is_some_and(|age| age >= STALE_LOCK_AGE)
}

fn unique_suffix() -> String {
    format!(
        "{}-{}-{}",
        std::process::id(),
        unix_nanos(),
        UNIQUE_COUNTER.fetch_add(1, Ordering::Relaxed)
    )
}

fn unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn unix_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
}

#[cfg(test)]
#[path = "fixture_cache_tests.rs"]
mod tests;
