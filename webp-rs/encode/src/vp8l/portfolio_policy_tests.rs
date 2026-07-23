use super::*;

fn cost(bits: usize, secondary: usize, runs: usize, memory: usize) -> PortfolioCost {
    PortfolioCost {
        exact_bits: bits,
        secondary_lookups: secondary,
        group_runs: runs,
        table_map_bytes: memory,
    }
}

#[test]
fn compact_never_spends_bits_for_decode_work() {
    let costs = [cost(10_000, 1_000, 1_000, 1_000), cost(10_001, 0, 0, 0)];
    assert_eq!(choose(SpatialProfile::Compact, &costs), Some(0));
}

#[test]
fn low_latency_has_a_bounded_one_percent_rate_envelope() {
    let costs = [
        cost(10_000, 10_000, 10_000, 100_000),
        cost(10_100, 0, 0, 0),
        cost(10_101, 0, 0, 0),
    ];
    assert_eq!(choose(SpatialProfile::LowLatency, &costs), Some(1));
}

#[test]
fn selection_is_stable_on_complete_ties() {
    let costs = [cost(80, 2, 3, 64), cost(80, 2, 3, 64)];
    assert_eq!(choose(SpatialProfile::Compact, &costs), Some(0));
    assert_eq!(choose(SpatialProfile::LowLatency, &costs), Some(0));
}
