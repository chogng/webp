use super::inverse_predictor_argb_reference;
use super::inverse_predictor_argb_to_rgba;
use super::inverse_predictor_rgba;
use crate::vp8l::header::BlockTransformDescriptor;
use crate::vp8l::pixel::argb_to_rgba;
use crate::vp8l::pixel::pack_argb;
use crate::vp8l::pixel::unpack_rgba;
use crate::vp8l::transforms::predictor::PredictorMode;
use crate::vp8l::transforms::predictor::Rgba;

#[test]
fn rgba_predictor_rows_match_the_scalar_reference_for_every_mode() {
    let descriptor = BlockTransformDescriptor {
        image_width: 3,
        image_height: 2,
        block_size_bits: 2,
        transform_width: 1,
        transform_height: 1,
    };
    let residuals = vec![
        0x1020_3040,
        0x5060_7080,
        0x90a0_b0c0,
        0xd0e0_f001,
        0x1234_5678,
        0x9abc_def0,
    ];

    for mode_value in 0_u8..=13 {
        let mode = PredictorMode::try_from(mode_value).unwrap();
        let mut expected = residuals.clone();
        for y in 0..2 {
            for x in 0..3 {
                let offset = y * 3 + x;
                let residual = argb_to_rgba(expected[offset]);
                let prediction = if x == 0 && y == 0 {
                    Rgba::OPAQUE_BLACK
                } else if y == 0 {
                    argb_to_rgba(expected[offset - 1])
                } else if x == 0 {
                    argb_to_rgba(expected[offset - 3])
                } else {
                    let left = argb_to_rgba(expected[offset - 1]);
                    let top = argb_to_rgba(expected[offset - 3]);
                    let top_left = argb_to_rgba(expected[offset - 4]);
                    let top_right = if x == 2 {
                        argb_to_rgba(expected[y * 3])
                    } else {
                        argb_to_rgba(expected[offset - 2])
                    };
                    crate::vp8l::transforms::predictor::predict(
                        mode, left, top, top_left, top_right,
                    )
                };
                expected[offset] = pack_argb(
                    residual.red.wrapping_add(prediction.red),
                    residual.green.wrapping_add(prediction.green),
                    residual.blue.wrapping_add(prediction.blue),
                    residual.alpha.wrapping_add(prediction.alpha),
                );
            }
        }

        let mut actual = residuals.clone();
        inverse_predictor_argb_reference(&mut actual, descriptor, &[u32::from(mode_value) << 8])
            .unwrap();
        assert_eq!(actual, expected, "predictor mode {mode_value}");

        let mut actual_rgba = Vec::with_capacity(residuals.len() * 4);
        for &pixel in &residuals {
            actual_rgba.extend_from_slice(&unpack_rgba(pixel));
        }
        inverse_predictor_rgba(&mut actual_rgba, descriptor, &[u32::from(mode_value) << 8])
            .unwrap();
        let actual_rgba = actual_rgba
            .chunks_exact(4)
            .map(|pixel| pack_argb(pixel[0], pixel[1], pixel[2], pixel[3]))
            .collect::<Vec<_>>();
        assert_eq!(actual_rgba, expected, "RGBA predictor mode {mode_value}");

        let fused =
            inverse_predictor_argb_to_rgba(&residuals, descriptor, &[u32::from(mode_value) << 8])
                .unwrap();
        let fused = fused
            .chunks_exact(4)
            .map(|pixel| pack_argb(pixel[0], pixel[1], pixel[2], pixel[3]))
            .collect::<Vec<_>>();
        assert_eq!(fused, expected, "fused predictor mode {mode_value}");
    }
}
