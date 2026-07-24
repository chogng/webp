//! Bounded VP8L match finding and spatial distance coding.

const MIN_MATCH_LENGTH: usize = 3;
const MAX_MATCH_LENGTH: usize = 4096;
const MAX_LINEAR_DISTANCE: usize = 1_048_456;
const MAX_HASH_BITS: u8 = 17;
pub(super) const DEFAULT_CHAIN_DEPTH: usize = 8;
pub(super) const DEEP_CHAIN_DEPTH: usize = 32;

const PLANE_TO_CODE: [u8; 128] = [
    96, 73, 55, 39, 23, 13, 5, 1, 255, 255, 255, 255, 255, 255, 255, 255, 101, 78, 58, 42, 26, 16,
    8, 2, 0, 3, 9, 17, 27, 43, 59, 79, 102, 86, 62, 46, 32, 20, 10, 6, 4, 7, 11, 21, 33, 47, 63,
    87, 105, 90, 70, 52, 37, 28, 18, 14, 12, 15, 19, 29, 38, 53, 71, 91, 110, 99, 82, 66, 48, 35,
    30, 24, 22, 25, 31, 36, 49, 67, 83, 100, 115, 108, 94, 76, 64, 50, 44, 40, 34, 41, 45, 51, 65,
    77, 95, 109, 118, 113, 103, 92, 80, 68, 60, 56, 54, 57, 61, 69, 81, 93, 104, 114, 119, 116,
    111, 106, 97, 88, 84, 74, 72, 75, 85, 89, 98, 107, 112, 117,
];

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct Match {
    pub(super) length: usize,
    pub(super) distance: usize,
}

pub(super) struct MatchFinder {
    heads: Vec<u32>,
    previous: Vec<u32>,
}

impl MatchFinder {
    pub(super) fn allocate(pixel_count: usize) -> Result<Self, std::collections::TryReserveError> {
        let hash_bits = usize::BITS
            .saturating_sub(pixel_count.max(1).leading_zeros())
            .clamp(8, u32::from(MAX_HASH_BITS)) as u8;
        let mut heads = Vec::new();
        heads.try_reserve_exact(1_usize << hash_bits)?;
        heads.resize(1_usize << hash_bits, u32::MAX);
        let mut previous = Vec::new();
        previous.try_reserve_exact(pixel_count)?;
        previous.resize(pixel_count, u32::MAX);
        Ok(Self { heads, previous })
    }

    pub(super) fn insert(&mut self, pixels: &[u32], index: usize) {
        if index + MIN_MATCH_LENGTH > pixels.len() {
            return;
        }
        let slot = match_hash(pixels, index) & (self.heads.len() - 1);
        self.previous[index] = self.heads[slot];
        self.heads[slot] = index as u32;
    }

    pub(super) fn find(&self, pixels: &[u32], index: usize, chain_depth: usize) -> Match {
        if index + MIN_MATCH_LENGTH > pixels.len() {
            return Match::default();
        }
        let slot = match_hash(pixels, index) & (self.heads.len() - 1);
        let mut candidate = self.heads[slot];
        let mut best = Match::default();
        let limit = MAX_MATCH_LENGTH.min(pixels.len() - index);
        for _ in 0..chain_depth {
            if candidate == u32::MAX {
                break;
            }
            let candidate_index = candidate as usize;
            let distance = index - candidate_index;
            if distance > MAX_LINEAR_DISTANCE {
                break;
            }
            if pixels[candidate_index..candidate_index + MIN_MATCH_LENGTH]
                == pixels[index..index + MIN_MATCH_LENGTH]
            {
                let mut length = MIN_MATCH_LENGTH;
                while length < limit && pixels[candidate_index + length] == pixels[index + length] {
                    length += 1;
                }
                if length > best.length || (length == best.length && distance < best.distance) {
                    best = Match { length, distance };
                    if length == limit {
                        break;
                    }
                }
            }
            candidate = self.previous[candidate_index];
        }
        best
    }
}

pub(super) fn distance_code(width: usize, distance: usize) -> usize {
    let y_offset = distance / width;
    let x_offset = distance - y_offset * width;
    if x_offset <= 8 && y_offset < 8 {
        usize::from(PLANE_TO_CODE[y_offset * 16 + 8 - x_offset]) + 1
    } else if x_offset + 8 > width && y_offset < 7 {
        usize::from(PLANE_TO_CODE[(y_offset + 1) * 16 + 8 + (width - x_offset)]) + 1
    } else {
        distance + 120
    }
}

fn match_hash(pixels: &[u32], index: usize) -> usize {
    let mut hash = pixels[index].wrapping_mul(0x1e35_a7bd);
    hash ^= pixels[index + 1].rotate_left(11);
    hash = hash.wrapping_mul(0x9e37_79b1);
    hash ^= pixels[index + 2].rotate_left(22);
    hash as usize
}

#[cfg(test)]
#[path = "lz77_tests.rs"]
mod tests;
