use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Barrier;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

use super::EnsureOutcome;
use super::current_marker;
use super::ensure;
use super::verify;
use crate::fixture_set::Fixture;

static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "webp-fixture-cache-test-{}-{}",
            std::process::id(),
            TEST_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn fixtures() -> Vec<Fixture> {
    vec![
        Fixture {
            name: "a.webp".to_owned(),
            bytes: b"first".to_vec(),
        },
        Fixture {
            name: "b.webp".to_owned(),
            bytes: b"second".to_vec(),
        },
    ]
}

#[test]
fn second_ensure_is_a_zero_publish_cache_hit() {
    let directory = TestDirectory::new();
    let root = directory.0.join("generated");
    let first = ensure(&root, &fixtures()).unwrap();
    assert_eq!(first.outcome, EnsureOutcome::Published);
    let marker = current_marker(&root).unwrap().path;
    let marker_modified = fs::metadata(&marker).unwrap().modified().unwrap();

    let second = ensure(&root, &fixtures()).unwrap();
    assert_eq!(second.outcome, EnsureOutcome::CacheHit);
    assert_eq!(second.digest, first.digest);
    assert_eq!(
        fs::metadata(marker).unwrap().modified().unwrap(),
        marker_modified
    );
}

#[test]
fn incomplete_staging_directory_is_never_selected() {
    let directory = TestDirectory::new();
    let root = directory.0.join("generated");
    fs::create_dir_all(root.join("sets/.staging-interrupted")).unwrap();
    fs::write(root.join("sets/.staging-interrupted/a.webp"), b"partial").unwrap();

    assert!(verify(&root, &fixtures()).is_err());
    assert_eq!(
        ensure(&root, &fixtures()).unwrap().outcome,
        EnsureOutcome::Published
    );
    verify(&root, &fixtures()).unwrap();
}

#[test]
fn corrupted_current_generation_is_quarantined_and_republished() {
    let directory = TestDirectory::new();
    let root = directory.0.join("generated");
    let summary = ensure(&root, &fixtures()).unwrap();
    let file = root.join("sets").join(&summary.digest).join("a.webp");
    fs::write(&file, b"corrupt").unwrap();
    assert!(verify(&root, &fixtures()).is_err());

    assert_eq!(
        ensure(&root, &fixtures()).unwrap().outcome,
        EnsureOutcome::Published
    );
    verify(&root, &fixtures()).unwrap();
    assert_eq!(fs::read(file).unwrap(), b"first");
}

#[test]
fn concurrent_ensure_publishes_one_complete_generation() {
    let directory = TestDirectory::new();
    let root = Arc::new(directory.0.join("generated"));
    let barrier = Arc::new(Barrier::new(4));
    let workers = (0..4)
        .map(|_| {
            let root = Arc::clone(&root);
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                ensure(&root, &fixtures()).unwrap().outcome
            })
        })
        .collect::<Vec<_>>();
    let outcomes = workers
        .into_iter()
        .map(|worker| worker.join().unwrap())
        .collect::<Vec<_>>();

    assert_eq!(
        outcomes
            .iter()
            .filter(|outcome| **outcome == EnsureOutcome::Published)
            .count(),
        1
    );
    verify(&root, &fixtures()).unwrap();
}
