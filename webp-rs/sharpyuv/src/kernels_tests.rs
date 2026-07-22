use super::filter_row;
use super::filter_row_scalar;
use super::update_rgb;
use super::update_rgb_scalar;
use super::update_y;
use super::update_y_scalar;

#[test]
fn vector_kernels_match_scalar_across_lengths_and_boundaries() {
    let mut random = Deterministic::new(0x4a91_d673_2fb8_c501);
    for len in 0..=65 {
        let target_y = samples_u16(len, &mut random);
        let reconstructed_y = samples_u16(len, &mut random);
        let original_y = samples_u16(len, &mut random);
        let mut scalar_y = original_y.clone();
        let mut vector_y = original_y;
        let scalar_difference = update_y_scalar(&target_y, &reconstructed_y, &mut scalar_y);
        let vector_difference = update_y(&target_y, &reconstructed_y, &mut vector_y);
        assert_eq!(
            vector_difference, scalar_difference,
            "Y difference at {len}"
        );
        assert_eq!(vector_y, scalar_y, "Y samples at {len}");

        let target_rgb = samples_i16(len, &mut random);
        let reconstructed_rgb = samples_i16(len, &mut random);
        let original_rgb = samples_i16(len, &mut random);
        let mut scalar_rgb = original_rgb.clone();
        let mut vector_rgb = original_rgb;
        update_rgb_scalar(&target_rgb, &reconstructed_rgb, &mut scalar_rgb);
        update_rgb(&target_rgb, &reconstructed_rgb, &mut vector_rgb);
        assert_eq!(vector_rgb, scalar_rgb, "RGB samples at {len}");

        let a = samples_i16(len + 1, &mut random);
        let b = samples_i16(len + 1, &mut random);
        let best_y = samples_u16(2 * len, &mut random);
        let mut scalar_output = vec![0; 2 * len];
        let mut vector_output = vec![0; 2 * len];
        filter_row_scalar(&a, &b, &best_y, &mut scalar_output);
        filter_row(&a, &b, &best_y, &mut vector_output);
        assert_eq!(vector_output, scalar_output, "filter samples at {len}");
    }
}

fn samples_u16(len: usize, random: &mut Deterministic) -> Vec<u16> {
    (0..len)
        .map(|index| match index % 11 {
            0 => 0,
            1 => 1023,
            _ => (random.next() & 1023) as u16,
        })
        .collect()
}

fn samples_i16(len: usize, random: &mut Deterministic) -> Vec<i16> {
    (0..len)
        .map(|index| match index % 13 {
            0 => -1023,
            1 => 1023,
            _ => ((random.next() & 2047) as i16) - 1023,
        })
        .collect()
}

struct Deterministic(u64);

impl Deterministic {
    const fn new(seed: u64) -> Self {
        Self(seed)
    }

    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}
