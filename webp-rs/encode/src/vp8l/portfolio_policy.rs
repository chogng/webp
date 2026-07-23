//! Deterministic rate/decode-work policy for the bounded spatial portfolio.

use super::spatial_plan::SpatialProfile;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PortfolioCost {
    pub(crate) exact_bits: usize,
    pub(crate) secondary_lookups: usize,
    pub(crate) group_runs: usize,
    pub(crate) table_map_bytes: usize,
}

impl PortfolioCost {
    fn score(self, profile: SpatialProfile) -> u128 {
        let (latency_weight, memory_weight) = match profile {
            // Compact admits no rate overhead, so these terms only provide a
            // deterministic tie-break between byte-identical candidates.
            SpatialProfile::Compact => (1_u128, 1_u128),
            // The minimum positive coefficient pair reached the eligible-set
            // decode oracle for both decoders on a disjoint 65-image
            // training/MustAccept/edge calibration. Larger coefficients did
            // not improve that result.
            SpatialProfile::LowLatency => (1, 1),
        };
        let decode_work =
            self.secondary_lookups as u128 + (self.group_runs as u128).saturating_mul(8);
        (self.exact_bits as u128)
            .saturating_add(decode_work.saturating_mul(latency_weight))
            .saturating_add((self.table_map_bytes as u128 / 64).saturating_mul(memory_weight))
    }
}

pub(super) fn choose(profile: SpatialProfile, costs: &[PortfolioCost]) -> Option<usize> {
    let rate_floor = costs.iter().map(|cost| cost.exact_bits).min()?;
    let rate_allowance = match profile {
        SpatialProfile::Compact => 0,
        SpatialProfile::LowLatency => rate_floor / 100,
    };
    let rate_ceiling = rate_floor.saturating_add(rate_allowance);
    costs
        .iter()
        .enumerate()
        .filter(|(_, cost)| cost.exact_bits <= rate_ceiling)
        .min_by_key(|(index, cost)| (cost.score(profile), cost.exact_bits, *index))
        .map(|(index, _)| index)
}

#[cfg(test)]
#[path = "portfolio_policy_tests.rs"]
mod tests;
