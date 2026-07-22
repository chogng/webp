//! Non-stable byte entry points for repository fuzzing and internal gates.

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
