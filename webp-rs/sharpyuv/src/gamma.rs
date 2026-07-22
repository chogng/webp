//! Fixed-point sRGB transfer tables used by the VP8 SharpYUV profile.

use std::sync::OnceLock;

const GAMMA_TO_LINEAR_SIZE: usize = 1 << 10;
const LINEAR_TO_GAMMA_SIZE: usize = 1 << 9;
const SCALE: f64 = 65_536.0;
const A: f64 = 0.099_296_826_809_44;
const THRESHOLD: f64 = 0.018_053_968_510_807;

struct GammaTables {
    gamma_to_linear: [u32; GAMMA_TO_LINEAR_SIZE + 2],
    linear_to_gamma: [u32; LINEAR_TO_GAMMA_SIZE + 2],
}

pub(super) fn gamma_to_linear(value: u16) -> u32 {
    gamma_tables().gamma_to_linear[usize::from(value)]
}

pub(super) fn linear_to_gamma(value: u32) -> u16 {
    fixed_point_interpolation(value, &gamma_tables().linear_to_gamma, 7, -6) as u16
}

fn fixed_point_interpolation(
    value: u32,
    table: &[u32],
    position_shift: u32,
    value_shift: i32,
) -> u32 {
    let position = value >> position_shift;
    let fraction = value - (position << position_shift);
    let first = shift(table[position as usize], value_shift);
    let second = shift(table[position as usize + 1], value_shift);
    first + (((second - first) * fraction + (1 << (position_shift - 1))) >> position_shift)
}

fn shift(value: u32, amount: i32) -> u32 {
    if amount >= 0 {
        value << amount
    } else {
        value >> -amount
    }
}

fn gamma_tables() -> &'static GammaTables {
    static TABLES: OnceLock<GammaTables> = OnceLock::new();
    TABLES.get_or_init(|| {
        let mut gamma_to_linear = [0; GAMMA_TO_LINEAR_SIZE + 2];
        for (value, output) in gamma_to_linear[..=GAMMA_TO_LINEAR_SIZE]
            .iter_mut()
            .enumerate()
        {
            let gamma = value as f64 / GAMMA_TO_LINEAR_SIZE as f64;
            let linear = if gamma <= THRESHOLD * 4.5 {
                gamma / 4.5
            } else {
                ((gamma + A) / (1.0 + A)).powf(1.0 / 0.45)
            };
            *output = (linear * SCALE + 0.5) as u32;
        }
        gamma_to_linear[GAMMA_TO_LINEAR_SIZE + 1] = gamma_to_linear[GAMMA_TO_LINEAR_SIZE];

        let mut linear_to_gamma = [0; LINEAR_TO_GAMMA_SIZE + 2];
        for (value, output) in linear_to_gamma[..=LINEAR_TO_GAMMA_SIZE]
            .iter_mut()
            .enumerate()
        {
            let linear = value as f64 / LINEAR_TO_GAMMA_SIZE as f64;
            let gamma = if linear <= THRESHOLD {
                linear * 4.5
            } else {
                (1.0 + A) * linear.powf(0.45) - A
            };
            *output = (gamma * SCALE + 0.5) as u32;
        }
        linear_to_gamma[LINEAR_TO_GAMMA_SIZE + 1] = linear_to_gamma[LINEAR_TO_GAMMA_SIZE];

        GammaTables {
            gamma_to_linear,
            linear_to_gamma,
        }
    })
}
