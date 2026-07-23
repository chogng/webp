//! Canonical VP8L entropy tokens, source geometry, and sufficient statistics.

use super::EncodeError;
use super::prefix::encode_prefix as vp8l_prefix;

pub(super) const GREEN_ALPHABET_SIZE: usize = 256 + 24;
pub(super) const CHANNEL_ALPHABET_SIZE: usize = 256;
pub(super) const DISTANCE_ALPHABET_SIZE: usize = 40;
pub(super) const MAX_ENCODER_COLOR_CACHE_BITS: u8 = 4;
const MAX_COLOR_CACHE_SIZE: usize = 1 << MAX_ENCODER_COLOR_CACHE_BITS;
pub(super) const FIRST_CACHE_SYMBOL: usize = GREEN_ALPHABET_SIZE;
pub(super) const MAIN_GREEN_ALPHABET_SIZE: usize = GREEN_ALPHABET_SIZE + MAX_COLOR_CACHE_SIZE;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum EntropyToken {
    Literal([u8; 4]),
    Cache(usize),
    Copy { length: usize },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct TokenGeometry {
    width: usize,
    height: usize,
    pixels: usize,
}

impl TokenGeometry {
    pub(super) const fn width(self) -> usize {
        self.width
    }

    pub(super) const fn height(self) -> usize {
        self.height
    }

    pub(super) const fn pixels(self) -> usize {
        self.pixels
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct TokenCensus {
    literal_tokens: usize,
    cache_tokens: usize,
    copy_tokens: usize,
    copied_pixels: usize,
    distance_symbols: usize,
}

impl TokenCensus {
    fn add_literal(&mut self) -> Result<(), EncodeError> {
        self.literal_tokens = checked_increment(self.literal_tokens)?;
        Ok(())
    }

    fn add_cache(&mut self) -> Result<(), EncodeError> {
        self.cache_tokens = checked_increment(self.cache_tokens)?;
        Ok(())
    }

    fn add_copy(&mut self, length: usize) -> Result<(), EncodeError> {
        self.copy_tokens = checked_increment(self.copy_tokens)?;
        self.copied_pixels = self
            .copied_pixels
            .checked_add(length)
            .ok_or_else(EncodeError::output_size_overflow)?;
        self.distance_symbols = checked_increment(self.distance_symbols)?;
        Ok(())
    }

    fn token_count(self) -> Result<usize, EncodeError> {
        self.literal_tokens
            .checked_add(self.cache_tokens)
            .and_then(|count| count.checked_add(self.copy_tokens))
            .ok_or_else(EncodeError::output_size_overflow)
    }

    fn pixel_count(self) -> Result<usize, EncodeError> {
        self.literal_tokens
            .checked_add(self.cache_tokens)
            .and_then(|count| count.checked_add(self.copied_pixels))
            .ok_or_else(EncodeError::output_size_overflow)
    }

    #[cfg(test)]
    pub(super) const fn literal_tokens(self) -> usize {
        self.literal_tokens
    }

    #[cfg(test)]
    pub(super) const fn cache_tokens(self) -> usize {
        self.cache_tokens
    }

    pub(super) const fn copy_tokens(self) -> usize {
        self.copy_tokens
    }

    #[cfg(test)]
    pub(super) const fn copied_pixels(self) -> usize {
        self.copied_pixels
    }

    pub(super) const fn distance_symbols(self) -> usize {
        self.distance_symbols
    }
}

pub(super) struct EntropyFrequencies {
    green: [u32; MAIN_GREEN_ALPHABET_SIZE],
    green_len: usize,
    red: [u32; CHANNEL_ALPHABET_SIZE],
    blue: [u32; CHANNEL_ALPHABET_SIZE],
    alpha: [u32; CHANNEL_ALPHABET_SIZE],
    distance: [u32; DISTANCE_ALPHABET_SIZE],
}

impl EntropyFrequencies {
    pub(super) fn for_color_cache(color_cache_bits: u8) -> Self {
        Self {
            green: [0; MAIN_GREEN_ALPHABET_SIZE],
            green_len: GREEN_ALPHABET_SIZE + color_cache_size(color_cache_bits),
            red: [0; CHANNEL_ALPHABET_SIZE],
            blue: [0; CHANNEL_ALPHABET_SIZE],
            alpha: [0; CHANNEL_ALPHABET_SIZE],
            distance: [0; DISTANCE_ALPHABET_SIZE],
        }
    }

    pub(super) fn add_token(&mut self, token: EntropyToken) -> Result<(), EncodeError> {
        match token {
            EntropyToken::Literal(rgba) => {
                increment_frequency(&mut self.green, usize::from(rgba[1]))?;
                increment_frequency(&mut self.red, usize::from(rgba[0]))?;
                increment_frequency(&mut self.blue, usize::from(rgba[2]))?;
                increment_frequency(&mut self.alpha, usize::from(rgba[3]))?;
            }
            EntropyToken::Cache(index) => {
                increment_frequency(&mut self.green, FIRST_CACHE_SYMBOL + index)?;
            }
            EntropyToken::Copy { length } => {
                let (length_prefix, _) = vp8l_prefix(length, 24)?;
                let (distance_prefix, _) = vp8l_prefix(121, DISTANCE_ALPHABET_SIZE)?;
                increment_frequency(&mut self.green, CHANNEL_ALPHABET_SIZE + length_prefix)?;
                increment_frequency(&mut self.distance, distance_prefix)?;
            }
        }
        Ok(())
    }

    pub(super) fn green(&self) -> &[u32] {
        &self.green[..self.green_len]
    }

    pub(super) const fn red(&self) -> &[u32; CHANNEL_ALPHABET_SIZE] {
        &self.red
    }

    pub(super) const fn blue(&self) -> &[u32; CHANNEL_ALPHABET_SIZE] {
        &self.blue
    }

    pub(super) const fn alpha(&self) -> &[u32; CHANNEL_ALPHABET_SIZE] {
        &self.alpha
    }

    pub(super) const fn distance(&self) -> &[u32; DISTANCE_ALPHABET_SIZE] {
        &self.distance
    }

    fn validate(&self, census: TokenCensus) -> Result<(), EncodeError> {
        if sum_frequencies(self.green())?
            != checked_u64_sum(
                census.literal_tokens,
                census.cache_tokens,
                census.copy_tokens,
            )?
            || sum_frequencies(self.red())? != census.literal_tokens as u64
            || sum_frequencies(self.blue())? != census.literal_tokens as u64
            || sum_frequencies(self.alpha())? != census.literal_tokens as u64
            || sum_frequencies(self.distance())? != census.distance_symbols as u64
        {
            return Err(EncodeError::output_size_overflow());
        }
        Ok(())
    }
}

pub(super) struct TokenStatistics {
    frequencies: EntropyFrequencies,
    census: TokenCensus,
}

impl TokenStatistics {
    pub(super) const fn frequencies(&self) -> &EntropyFrequencies {
        &self.frequencies
    }

    pub(super) const fn census(&self) -> TokenCensus {
        self.census
    }
}

/// The single owner of canonical tokens and statistics for one entropy image.
///
/// Tokens are in source-pixel order. A copy is owned by the block containing
/// its first pixel and may span later blocks. The census and channel
/// frequencies describe exactly this token sequence.
pub(crate) struct TokenStream {
    geometry: TokenGeometry,
    color_cache_bits: u8,
    tokens: Vec<EntropyToken>,
    statistics: TokenStatistics,
}

impl TokenStream {
    pub(crate) fn collect(
        rgba: &[u8],
        width: usize,
        use_subtract_green: bool,
        use_left_predictor: bool,
        color_cache_bits: u8,
    ) -> Result<Self, EncodeError> {
        let pixels = rgba.len() / 4;
        if width == 0
            || pixels == 0
            || !rgba.len().is_multiple_of(4)
            || !pixels.is_multiple_of(width)
        {
            return Err(EncodeError::output_size_overflow());
        }
        if color_cache_bits > MAX_ENCODER_COLOR_CACHE_BITS {
            return Err(EncodeError::output_size_overflow());
        }
        let geometry = TokenGeometry {
            width,
            height: pixels / width,
            pixels,
        };
        let mut tokens = Vec::new();
        tokens
            .try_reserve_exact(pixels)
            .map_err(|_| EncodeError::allocation_failed())?;
        let mut frequencies = EntropyFrequencies::for_color_cache(color_cache_bits);
        let mut census = TokenCensus::default();
        let mut color_cache = [0_u32; MAX_COLOR_CACHE_SIZE];
        let mut residuals = Vec::new();
        residuals
            .try_reserve_exact(pixels)
            .map_err(|_| EncodeError::allocation_failed())?;
        for index in 0..pixels {
            let current = if use_subtract_green {
                subtract_green_pixel(rgba, index)
            } else {
                pixel_at(rgba, index)
            };
            let predictor = if use_left_predictor {
                left_predictor(rgba, index, width)
            } else {
                [0; 4]
            };
            residuals.push([
                current[0].wrapping_sub(predictor[0]),
                current[1].wrapping_sub(predictor[1]),
                current[2].wrapping_sub(predictor[2]),
                current[3].wrapping_sub(predictor[3]),
            ]);
        }

        let mut index = 0_usize;
        while index < residuals.len() {
            let residual = residuals[index];
            if index != 0 && residual == residuals[index - 1] {
                let mut length = 1_usize;
                while length < 4096
                    && index + length < residuals.len()
                    && residuals[index + length] == residual
                {
                    length += 1;
                }
                if length >= 3 {
                    let token = EntropyToken::Copy { length };
                    frequencies.add_token(token)?;
                    census.add_copy(length)?;
                    for _ in 0..length {
                        update_color_cache(&mut color_cache, color_cache_bits, pack_argb(residual));
                    }
                    tokens.push(token);
                    index += length;
                    continue;
                }
            }
            let color = pack_argb(residual);
            let cache_index = if color_cache_bits == 0 {
                0
            } else {
                color_cache_index(color, color_cache_bits)
            };
            let token = if color_cache_bits != 0 && color_cache[cache_index] == color {
                census.add_cache()?;
                EntropyToken::Cache(cache_index)
            } else {
                census.add_literal()?;
                EntropyToken::Literal(residual)
            };
            frequencies.add_token(token)?;
            tokens.push(token);
            color_cache[cache_index] = color;
            index += 1;
        }

        if census.token_count()? != tokens.len()
            || census.pixel_count()? != geometry.pixels()
            || census.distance_symbols != census.copy_tokens
        {
            return Err(EncodeError::output_size_overflow());
        }
        frequencies.validate(census)?;
        Ok(Self {
            geometry,
            color_cache_bits,
            tokens,
            statistics: TokenStatistics {
                frequencies,
                census,
            },
        })
    }

    pub(super) const fn geometry(&self) -> TokenGeometry {
        self.geometry
    }

    pub(super) const fn color_cache_bits(&self) -> u8 {
        self.color_cache_bits
    }

    pub(crate) fn tokens(&self) -> &[EntropyToken] {
        &self.tokens
    }

    pub(super) const fn statistics(&self) -> &TokenStatistics {
        &self.statistics
    }
}

pub(super) const fn token_span(token: EntropyToken) -> usize {
    match token {
        EntropyToken::Literal(_) | EntropyToken::Cache(_) => 1,
        EntropyToken::Copy { length } => length,
    }
}

pub(crate) fn select_color_cache_bits(
    rgba: &[u8],
    width: usize,
    use_subtract_green: bool,
    use_left_predictor: bool,
) -> u8 {
    let mut selected_bits = 0;
    let mut best_hits = 0_u32;
    for bits in 1..=MAX_ENCODER_COLOR_CACHE_BITS {
        let mut cache = [0_u32; MAX_COLOR_CACHE_SIZE];
        let mut hits = 0_u32;
        for index in 0..rgba.len() / 4 {
            let current = if use_subtract_green {
                subtract_green_pixel(rgba, index)
            } else {
                pixel_at(rgba, index)
            };
            let predictor = if use_left_predictor {
                left_predictor(rgba, index, width)
            } else {
                [0; 4]
            };
            let residual = [
                current[0].wrapping_sub(predictor[0]),
                current[1].wrapping_sub(predictor[1]),
                current[2].wrapping_sub(predictor[2]),
                current[3].wrapping_sub(predictor[3]),
            ];
            let color = pack_argb(residual);
            let cache_index = color_cache_index(color, bits);
            if cache[cache_index] == color {
                hits = hits.saturating_add(1);
            }
            cache[cache_index] = color;
        }
        if hits > best_hits {
            best_hits = hits;
            selected_bits = bits;
        }
    }
    selected_bits
}

pub(crate) fn select_left_predictor(rgba: &[u8], width: usize) -> bool {
    let mut matching_neighbours = 0_usize;
    for index in 1..rgba.len() / 4 {
        let current = subtract_green_pixel(rgba, index);
        let predictor = left_predictor(rgba, index, width);
        if current == predictor {
            matching_neighbours += 1;
        }
    }
    matching_neighbours.saturating_mul(16) >= rgba.len() / 4
}

const fn color_cache_size(bits: u8) -> usize {
    if bits == 0 { 0 } else { 1 << bits }
}

fn color_cache_index(color: u32, bits: u8) -> usize {
    debug_assert!(bits != 0 && bits <= MAX_ENCODER_COLOR_CACHE_BITS);
    hash_color(color, bits)
}

const fn hash_color(color: u32, bits: u8) -> usize {
    let shift = u32::BITS - bits as u32;
    (color.wrapping_mul(0x1e35_a7bd) >> shift) as usize
}

fn update_color_cache(cache: &mut [u32; MAX_COLOR_CACHE_SIZE], bits: u8, color: u32) {
    if bits != 0 {
        cache[color_cache_index(color, bits)] = color;
    }
}

fn subtract_green_pixel(rgba: &[u8], index: usize) -> [u8; 4] {
    let [red, green, blue, alpha] = pixel_at(rgba, index);
    [
        red.wrapping_sub(green),
        green,
        blue.wrapping_sub(green),
        alpha,
    ]
}

fn pixel_at(rgba: &[u8], index: usize) -> [u8; 4] {
    let offset = index * 4;
    [
        rgba[offset],
        rgba[offset + 1],
        rgba[offset + 2],
        rgba[offset + 3],
    ]
}

fn left_predictor(rgba: &[u8], index: usize, width: usize) -> [u8; 4] {
    if index == 0 {
        return [0, 0, 0, u8::MAX];
    }
    let x = index % width;
    let predictor_index = if x == 0 { index - width } else { index - 1 };
    subtract_green_pixel(rgba, predictor_index)
}

fn pack_argb(rgba: [u8; 4]) -> u32 {
    (u32::from(rgba[3]) << 24)
        | (u32::from(rgba[0]) << 16)
        | (u32::from(rgba[1]) << 8)
        | u32::from(rgba[2])
}

fn increment_frequency(table: &mut [u32], symbol: usize) -> Result<(), EncodeError> {
    let frequency = table
        .get_mut(symbol)
        .ok_or_else(EncodeError::output_size_overflow)?;
    *frequency = frequency
        .checked_add(1)
        .ok_or_else(EncodeError::output_size_overflow)?;
    Ok(())
}

fn checked_increment(value: usize) -> Result<usize, EncodeError> {
    value
        .checked_add(1)
        .ok_or_else(EncodeError::output_size_overflow)
}

fn checked_u64_sum(first: usize, second: usize, third: usize) -> Result<u64, EncodeError> {
    first
        .checked_add(second)
        .and_then(|sum| sum.checked_add(third))
        .and_then(|sum| u64::try_from(sum).ok())
        .ok_or_else(EncodeError::output_size_overflow)
}

fn sum_frequencies(frequencies: &[u32]) -> Result<u64, EncodeError> {
    frequencies.iter().try_fold(0_u64, |sum, &frequency| {
        sum.checked_add(u64::from(frequency))
            .ok_or_else(EncodeError::output_size_overflow)
    })
}

#[cfg(test)]
#[path = "token_stream_tests.rs"]
mod tests;
