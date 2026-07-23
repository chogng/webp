use super::*;

#[test]
fn bounded_chain_finds_non_adjacent_and_overlapping_matches() {
    let pixels = [1, 2, 3, 4, 9, 1, 2, 3, 4, 1, 2, 3, 4];
    let mut finder = MatchFinder::allocate(pixels.len()).expect("allocate match finder");
    for index in 0..5 {
        finder.insert(&pixels, index);
    }
    assert_eq!(
        finder.find(&pixels, 5),
        Match {
            length: 4,
            distance: 5,
        }
    );
    for index in 5..9 {
        finder.insert(&pixels, index);
    }
    assert_eq!(
        finder.find(&pixels, 9),
        Match {
            length: 4,
            distance: 4,
        }
    );
}

#[test]
fn distance_codes_round_trip_representative_plane_and_linear_offsets() {
    assert_eq!(distance_code(32, 1), 2);
    assert_eq!(distance_code(32, 32), 1);
    assert_eq!(distance_code(32, 33), 3);
    assert_eq!(distance_code(32, 31), 4);
    assert_eq!(distance_code(3, 97), 217);
}
