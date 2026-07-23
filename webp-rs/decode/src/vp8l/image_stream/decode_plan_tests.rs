use super::*;

fn limits() -> DecodeLimits {
    DecodeLimits {
        max_width: 16_384,
        max_height: 16_384,
        max_pixels: 16_384 * 16_384,
        ..DecodeLimits::default()
    }
}

#[test]
fn plan_validates_geometry_and_owns_single_backing_census() {
    let header = Vp8lHeader {
        width: 7,
        height: 3,
        alpha_is_used: true,
        version: 0,
    };
    let plan = DecodePlan::build(
        header,
        DecodedTransformList {
            transforms: vec![DecodedTransform::SubtractGreen],
            coded_width: 7,
            coded_height: 3,
        },
        12,
        &limits(),
    )
    .expect("validated plan");
    assert_eq!(plan.coded_pixels(), 21);
    assert_eq!(plan.rgba_len(), 84);
    assert_eq!(plan.retained_transform_bytes(), 12);
    assert_eq!(plan.kernel(), KernelFamily::ScalarRgba);
    assert_eq!(
        plan.storage(),
        DecodeStorageCensus {
            full_image_allocations: 1,
            full_image_copy_bytes: 0,
            peak_image_backing_bytes: 84,
        }
    );
}

#[test]
fn plan_rejects_coded_or_terminal_geometry_mismatch() {
    let header = Vp8lHeader {
        width: 2,
        height: 2,
        alpha_is_used: false,
        version: 0,
    };
    for decoded in [
        DecodedTransformList {
            transforms: vec![],
            coded_width: 3,
            coded_height: 2,
        },
        DecodedTransformList {
            transforms: vec![],
            coded_width: 1,
            coded_height: 2,
        },
    ] {
        assert_eq!(
            DecodePlan::build(header, decoded, 0, &limits())
                .err()
                .expect("invalid transform geometry should fail")
                .kind(),
            DecodeErrorKind::InvalidBitstream
        );
    }
}
