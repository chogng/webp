//! Bounded LZ77 parsing and VP8L distance coding for alpha payload writing.

const MIN_MATCH_LENGTH: usize = 4;
const MAX_MATCH_LENGTH: usize = 4096;
const MATCH_HASH_SIZE: usize = 1 << 16;
const MAX_LINEAR_DISTANCE: usize = 1_048_456;
pub(super) const MAX_CACHED_TOKEN_SAMPLES: usize = 4 * 1024 * 1024;
pub(super) const CHANNEL_ALPHABET_SIZE: usize = 256;
pub(super) const LENGTH_PREFIX_COUNT: usize = 24;
pub(super) const GREEN_ALPHABET_SIZE: usize = CHANNEL_ALPHABET_SIZE + LENGTH_PREFIX_COUNT;
pub(super) const DISTANCE_ALPHABET_SIZE: usize = 40;

const PLANE_TO_CODE: [u8; 128] = [
    96, 73, 55, 39, 23, 13, 5, 1, 255, 255, 255, 255, 255, 255, 255, 255, 101, 78, 58, 42, 26, 16,
    8, 2, 0, 3, 9, 17, 27, 43, 59, 79, 102, 86, 62, 46, 32, 20, 10, 6, 4, 7, 11, 21, 33, 47, 63,
    87, 105, 90, 70, 52, 37, 28, 18, 14, 12, 15, 19, 29, 38, 53, 71, 91, 110, 99, 82, 66, 48, 35,
    30, 24, 22, 25, 31, 36, 49, 67, 83, 100, 115, 108, 94, 76, 64, 50, 44, 40, 34, 41, 45, 51, 65,
    77, 95, 109, 118, 113, 103, 92, 80, 68, 60, 56, 54, 57, 61, 69, 81, 93, 104, 114, 119, 116,
    111, 106, 97, 88, 84, 74, 72, 75, 85, 89, 98, 107, 112, 117,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Token {
    Literal(u8),
    Copy { length: usize, distance: usize },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct PrefixCode {
    pub(super) symbol: usize,
    pub(super) extra: u32,
    pub(super) extra_bits: u8,
}

pub(super) fn prefix_code(value: usize, prefix_count: usize) -> Option<PrefixCode> {
    for prefix in 0..prefix_count {
        if prefix < 4 {
            if value == prefix + 1 {
                return Some(PrefixCode {
                    symbol: prefix,
                    extra: 0,
                    extra_bits: 0,
                });
            }
            continue;
        }
        let prefix_byte = u8::try_from(prefix).ok()?;
        let extra_bits = (prefix_byte - 2) >> 1;
        let offset = (2_usize + usize::from(prefix_byte & 1)) << extra_bits;
        let minimum = offset.checked_add(1)?;
        let maximum = minimum.checked_add((1_usize << extra_bits) - 1)?;
        if (minimum..=maximum).contains(&value) {
            return Some(PrefixCode {
                symbol: prefix,
                extra: u32::try_from(value - minimum).ok()?,
                extra_bits,
            });
        }
    }
    None
}

pub(super) struct MatchTable {
    heads: Vec<u32>,
}

impl MatchTable {
    pub(super) fn allocate(sample_count: usize) -> Result<Self, std::collections::TryReserveError> {
        let slot_count = sample_count
            .next_power_of_two()
            .saturating_mul(2)
            .clamp(256, MATCH_HASH_SIZE);
        let mut heads = Vec::new();
        heads.try_reserve_exact(slot_count)?;
        heads.resize(slot_count, u32::MAX);
        Ok(Self { heads })
    }

    pub(super) fn reset(&mut self) {
        self.heads.fill(u32::MAX);
    }

    fn insert(&mut self, hash: usize, index: usize) {
        self.heads[hash] = index as u32;
    }

    fn hash(&self, samples: &[u8], index: usize) -> usize {
        match_hash(samples, index) & (self.heads.len() - 1)
    }
}

pub(super) fn allocate_token_cache(sample_count: usize) -> Option<Vec<u32>> {
    if sample_count > MAX_CACHED_TOKEN_SAMPLES {
        return None;
    }
    let mut tokens = Vec::new();
    tokens.try_reserve_exact(sample_count).ok()?;
    Some(tokens)
}

pub(super) fn pack(token: Token) -> u32 {
    match token {
        Token::Literal(sample) => u32::from(sample),
        Token::Copy { length, distance } => {
            debug_assert!((1..=MAX_MATCH_LENGTH).contains(&length));
            debug_assert!((1..=MAX_LINEAR_DISTANCE).contains(&distance));
            ((distance as u32) << 12) | (length as u32 - 1)
        }
    }
}

pub(super) fn unpack(token: u32) -> Token {
    let distance = (token >> 12) as usize;
    if distance == 0 {
        Token::Literal(token as u8)
    } else {
        Token::Copy {
            length: ((token & 0x0fff) + 1) as usize,
            distance,
        }
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

pub(super) fn walk<E>(
    samples: &[u8],
    table: &mut MatchTable,
    mut emit: impl FnMut(Token) -> Result<(), E>,
) -> Result<(), E> {
    let mut index = 0_usize;
    while index < samples.len() {
        let best = find_match(samples, table, index);
        if index + MIN_MATCH_LENGTH <= samples.len() {
            table.insert(table.hash(samples, index), index);
        }
        if best.length >= MIN_MATCH_LENGTH {
            emit(Token::Copy {
                length: best.length,
                distance: best.distance,
            })?;
            for skipped in index + 1..index + best.length {
                if skipped + MIN_MATCH_LENGTH <= samples.len() {
                    table.insert(table.hash(samples, skipped), skipped);
                }
            }
            index += best.length;
        } else {
            emit(Token::Literal(samples[index]))?;
            index += 1;
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Default)]
struct Match {
    length: usize,
    distance: usize,
}

fn find_match(samples: &[u8], table: &MatchTable, index: usize) -> Match {
    if index + MIN_MATCH_LENGTH > samples.len() {
        return Match::default();
    }
    let candidate = table.heads[table.hash(samples, index)];
    if candidate == u32::MAX {
        return Match::default();
    }
    let candidate = candidate as usize;
    let distance = index - candidate;
    if distance == 0
        || distance > MAX_LINEAR_DISTANCE
        || samples[candidate..candidate + MIN_MATCH_LENGTH]
            != samples[index..index + MIN_MATCH_LENGTH]
    {
        return Match::default();
    }
    let limit = MAX_MATCH_LENGTH.min(samples.len() - index);
    let mut length = MIN_MATCH_LENGTH;
    while length < limit && samples[candidate + length] == samples[index + length] {
        length += 1;
    }
    Match { length, distance }
}

fn match_hash(samples: &[u8], index: usize) -> usize {
    let word = u32::from(samples[index])
        | (u32::from(samples[index + 1]) << 8)
        | (u32::from(samples[index + 2]) << 16);
    ((word.wrapping_mul(0x1e35_a7bd)) >> 16) as usize
}

#[cfg(test)]
#[path = "backward_references_tests.rs"]
mod tests;
