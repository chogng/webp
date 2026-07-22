#[cfg(test)]
#[derive(Default)]
pub(in crate::vp8l) struct DecodePhaseTimings {
    pub(in crate::vp8l) entropy: std::time::Duration,
    pub(in crate::vp8l) rgba_conversion: std::time::Duration,
    pub(in crate::vp8l) predictor: std::time::Duration,
    pub(in crate::vp8l) entropy_paths: EntropyPathCounters,
}

#[cfg(test)]
#[derive(Clone, Copy, Default)]
pub(in crate::vp8l) struct EntropyPathCounters {
    pub(in crate::vp8l) literal_pixels: u64,
    pub(in crate::vp8l) batched_literals: u64,
    pub(in crate::vp8l) cache_hits: u64,
    pub(in crate::vp8l) copy_commands: u64,
    pub(in crate::vp8l) copy_pixels: u64,
    pub(in crate::vp8l) meta_runs: u64,
}

#[cfg(test)]
impl EntropyPathCounters {
    pub(in crate::vp8l) fn add_assign(&mut self, other: Self) {
        self.literal_pixels += other.literal_pixels;
        self.batched_literals += other.batched_literals;
        self.cache_hits += other.cache_hits;
        self.copy_commands += other.copy_commands;
        self.copy_pixels += other.copy_pixels;
        self.meta_runs += other.meta_runs;
    }
}

#[cfg(test)]
std::thread_local! {
    static ENTROPY_PATH_COUNTERS: std::cell::Cell<EntropyPathCounters> =
        const { std::cell::Cell::new(EntropyPathCounters {
            literal_pixels: 0,
            batched_literals: 0,
            cache_hits: 0,
            copy_commands: 0,
            copy_pixels: 0,
            meta_runs: 0,
        }) };
}

#[cfg(test)]
pub(in crate::vp8l) fn reset_entropy_path_counters() {
    ENTROPY_PATH_COUNTERS.with(|counters| counters.set(EntropyPathCounters::default()));
}

#[cfg(test)]
pub(in crate::vp8l) fn entropy_path_counters() -> EntropyPathCounters {
    ENTROPY_PATH_COUNTERS.with(std::cell::Cell::get)
}

#[cfg(test)]
pub(in crate::vp8l) fn record_entropy_path(update: impl FnOnce(&mut EntropyPathCounters)) {
    ENTROPY_PATH_COUNTERS.with(|counters| {
        let mut current = counters.get();
        update(&mut current);
        counters.set(current);
    });
}
