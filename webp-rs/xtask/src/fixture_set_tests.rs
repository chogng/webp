use std::collections::BTreeSet;

use super::generate;

#[test]
fn generated_set_has_unique_safe_names_and_consistent_riff_lengths() {
    let fixtures = generate();
    let mut names = BTreeSet::new();
    for fixture in fixtures {
        assert!(names.insert(fixture.name.clone()), "duplicate fixture name");
        assert!(!fixture.name.contains(['/', '\\', '\n', '\r', ' ']));
        assert!(fixture.name.ends_with(".webp"));
        assert_eq!(&fixture.bytes[..4], b"RIFF");
        if fixture.name.starts_with("metadata-") && fixture.bytes.len() >= 12 {
            let declared = u32::from_le_bytes(fixture.bytes[4..8].try_into().unwrap()) as usize;
            assert_eq!(declared + 8, fixture.bytes.len());
            assert_eq!(&fixture.bytes[8..12], b"WEBP");
        }
    }
}
