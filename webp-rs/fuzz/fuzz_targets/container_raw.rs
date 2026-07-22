#![no_main]
#![forbid(unsafe_code)]

use libfuzzer_sys::fuzz_target;
use webp_container::CompatibilityProfile;
use webp_container::ContainerLimits;

fuzz_target!(|bytes: &[u8]| {
    let limits = ContainerLimits {
        max_input_bytes: 1 << 20,
        max_width: 4096,
        max_height: 4096,
        max_pixels: 16 * 1024 * 1024,
        max_metadata_bytes: 1 << 20,
        ..ContainerLimits::default()
    };
    let _ = webp_container::parse(bytes, CompatibilityProfile::SpecStrict, &limits);
});
