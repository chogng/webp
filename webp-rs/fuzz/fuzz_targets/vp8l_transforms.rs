#![no_main]
#![forbid(unsafe_code)]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|input: &[u8]| webp::fuzzing::vp8l_transforms(input));
