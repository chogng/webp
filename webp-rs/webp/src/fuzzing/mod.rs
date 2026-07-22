//! Non-stable byte entry points for repository fuzzing and internal gates.

use crate::AlphaEncodeOptions;

#[cfg(feature = "alpha-benchmark-internals")]
#[doc(hidden)]
pub use crate::alpha::BenchmarkWriterVariant;
#[cfg(feature = "alpha-benchmark-internals")]
#[doc(hidden)]
pub use crate::alpha::set_benchmark_writer_variant;

pub fn vp8_bool(input: &[u8]) {
    crate::vp8::fuzzing::bool_coder(input);
}
pub fn vp8_coefficients(input: &[u8]) {
    crate::vp8::fuzzing::coefficients(input);
}
pub fn vp8_partition(input: &[u8]) {
    crate::vp8::fuzzing::partition(input);
}
pub fn vp8_residuals(input: &[u8]) {
    crate::vp8::fuzzing::residuals(input);
}
pub fn vp8_transforms(input: &[u8]) {
    crate::vp8::fuzzing::transforms(input);
}
pub fn vp8l_huffman(input: &[u8]) {
    crate::vp8l::fuzzing::huffman(input);
}
pub fn vp8l_transforms(input: &[u8]) {
    crate::vp8l::fuzzing::transforms(input);
}

/// Encodes a header-bearing ALPH payload for the repository's performance gate.
pub fn encode_alpha(
    samples: &[u8],
    width: u32,
    height: u32,
    options: AlphaEncodeOptions,
) -> Result<Vec<u8>, crate::alpha::AlphaEncodeError> {
    crate::alpha::encode(samples, width, height, options)
}

/// Converts one RGBA image through the private SharpYUV path and hashes only
/// its visible Y, U, and V samples for repository performance gates.
///
/// This is deliberately available only with the non-stable `fuzzing` feature.
/// It is not a public color-conversion API.
pub fn sharp_yuv420_visible_checksum(width: u32, height: u32, rgba: &[u8]) -> Option<u64> {
    let image = crate::vp8::rgba_to_yuv420(width, height, rgba).ok()?;
    let width = usize::try_from(width).ok()?;
    let height = usize::try_from(height).ok()?;
    let uv_width = width.div_ceil(2);
    let uv_height = height.div_ceil(2);
    let mut checksum = 14_695_981_039_346_656_037_u64;
    checksum = hash_bytes(checksum, &(width as u64).to_le_bytes());
    checksum = hash_bytes(checksum, &(height as u64).to_le_bytes());
    for row in 0..height {
        checksum = hash_bytes(
            checksum,
            &image.y[row * image.y_stride..row * image.y_stride + width],
        );
    }
    for row in 0..uv_height {
        checksum = hash_bytes(
            checksum,
            &image.u[row * image.uv_stride..row * image.uv_stride + uv_width],
        );
    }
    for row in 0..uv_height {
        checksum = hash_bytes(
            checksum,
            &image.v[row * image.uv_stride..row * image.uv_stride + uv_width],
        );
    }
    Some(checksum)
}

fn hash_bytes(mut checksum: u64, bytes: &[u8]) -> u64 {
    for byte in bytes {
        checksum ^= u64::from(*byte);
        checksum = checksum.wrapping_mul(1_099_511_628_211);
    }
    checksum
}
