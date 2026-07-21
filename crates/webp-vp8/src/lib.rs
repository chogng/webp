#![forbid(unsafe_code)]
//! VP8 lossy WebP decoder primitives.

mod bitstream;
mod coefficients;
mod entropy;
mod frame;
mod intra;
mod loop_filter;
mod partition;
mod quantization;
mod reconstruction;
#[cfg(test)]
mod test_support;
mod transform;

pub use bitstream::BoolDecoder;
pub use coefficients::COEFFICIENT_ZIGZAG;
pub use entropy::{
    CoefficientBlockType, CoefficientProbabilities, DecodedCoefficients, MacroblockResiduals,
    ResidualContext, decode_coefficients, decode_intra_residuals,
};
pub use frame::{Vp8YuvImage, decode_intra_frame};
pub use intra::{
    ChromaMode, Intra4Mode, Intra16Mode, IntraMacroblock, LumaMode, parse_intra_mode_row,
};
pub use loop_filter::{
    LoopFilterStrength, derive_loop_filter_strengths, filter_normal_edge, filter_simple_edge,
};
pub use partition::{
    FilterHeader, FirstPartitionHeader, PartitionLayout, SegmentHeader, TokenPartition, Vp8Header,
    parse_partition_layout, parse_riff_payload,
};
pub use quantization::{DequantizationMatrix, QuantizationHeader, derive_dequantization};
pub use reconstruction::{
    DequantizedMacroblock, MacroblockPixels, MacroblockPredictionEdges, MacroblockSpatialResidues,
    add_residue_and_clip, combine_macroblock_prediction, dequantize_macroblock,
    inverse_transform_macroblock, predict_intra4_block, predict_intra4_macroblock,
    predict_intra16_macroblock, reconstruct_intra_macroblock,
};
pub use transform::{inverse_dct_4x4, inverse_dct_4x4_i32, inverse_wht_4x4, inverse_wht_4x4_i32};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coefficients::{CATEGORY_PROBABILITIES, COEFFICIENT_UPDATE_PROBABILITIES};
    use crate::partition::{KEY_FRAME_HEADER_LEN, KEY_FRAME_START_CODE};
    use webp_core::{DecodeErrorKind, DecodeLimits};

    /// A deliberately straightforward VP8 boolean writer used only to produce
    /// independently driven decoder vectors. It follows the encoder interval
    /// update and byte-flush rules, not the decoder's cached-value structure.
    #[derive(Default)]
    struct TestBoolWriter {
        range: i32,
        value: i32,
        run: usize,
        pending_bits: i32,
        bytes: Vec<u8>,
    }

    impl TestBoolWriter {
        fn new() -> Self {
            Self {
                range: 254,
                value: 0,
                run: 0,
                pending_bits: -8,
                bytes: Vec::new(),
            }
        }

        fn write_bool(&mut self, bit: bool, probability: u8) {
            let split = (self.range * i32::from(probability)) >> 8;
            if bit {
                self.value += split + 1;
                self.range -= split + 1;
            } else {
                self.range = split;
            }
            if self.range < 127 {
                let shift = if self.range == 0 {
                    7
                } else {
                    7 - self.range.ilog2() as i32
                };
                self.range = ((self.range + 1) << shift) - 1;
                self.value <<= shift;
                self.pending_bits += shift;
                if self.pending_bits > 0 {
                    self.flush();
                }
            }
        }

        fn write_literal(&mut self, value: u32, count: u8) {
            for shift in (0..count).rev() {
                self.write_bool(((value >> shift) & 1) != 0, 128);
            }
        }

        fn write_signed_literal(&mut self, value: i32, count: u8) {
            self.write_literal(value.unsigned_abs(), count);
            self.write_bool(value.is_negative(), 128);
        }

        fn finish(mut self) -> Vec<u8> {
            self.write_literal(0, (9 - self.pending_bits) as u8);
            self.pending_bits = 0;
            self.flush();
            self.bytes
        }

        fn flush(&mut self) {
            let shift = 8 + self.pending_bits;
            let bits = self.value >> shift;
            self.value -= bits << shift;
            self.pending_bits -= 8;
            if bits & 0xff == 0xff {
                self.run += 1;
                return;
            }
            if bits & 0x100 != 0
                && let Some(previous) = self.bytes.last_mut()
            {
                *previous += 1;
            }
            let delayed = if bits & 0x100 != 0 { 0 } else { 0xff };
            self.bytes.extend(std::iter::repeat_n(delayed, self.run));
            self.run = 0;
            self.bytes.push((bits & 0xff) as u8);
        }
    }

    fn write_quantization_header(
        writer: &mut TestBoolWriter,
        base_index: u8,
        deltas: [i32; 5],
        refresh_entropy_probabilities: bool,
    ) {
        writer.write_literal(u32::from(base_index), 7);
        for value in deltas {
            writer.write_bool(value != 0, 128);
            if value != 0 {
                writer.write_signed_literal(value, 4);
            }
        }
        writer.write_bool(refresh_entropy_probabilities, 128);
    }

    fn write_coefficient_updates(
        writer: &mut TestBoolWriter,
        updates: &[(usize, usize, usize, usize, u8)],
        use_skip_probability: bool,
        skip_probability: u8,
    ) {
        for (coefficient_type, bands) in COEFFICIENT_UPDATE_PROBABILITIES.iter().enumerate() {
            for (band, contexts) in bands.iter().enumerate() {
                for (context, nodes) in contexts.iter().enumerate() {
                    for (node, &update_probability) in nodes.iter().enumerate() {
                        let update = updates.iter().find(|&&(t, b, c, n, _)| {
                            (t, b, c, n) == (coefficient_type, band, context, node)
                        });
                        writer.write_bool(update.is_some(), update_probability);
                        if let Some(&(_, _, _, _, value)) = update {
                            writer.write_literal(u32::from(value), 8);
                        }
                    }
                }
            }
        }
        writer.write_bool(use_skip_probability, 128);
        if use_skip_probability {
            writer.write_literal(u32::from(skip_probability), 8);
        }
    }

    fn pad_first_partition(writer: &mut TestBoolWriter) {
        writer.write_literal(0, 8); // Leave structural fields away from EOF.
    }

    fn coefficient_nodes(
        probabilities: &CoefficientProbabilities,
        coefficient_type: CoefficientBlockType,
        position: usize,
        context: usize,
    ) -> &[u8; 11] {
        probabilities.nodes(coefficient_type, position, context)
    }

    fn write_coefficient_eob(
        writer: &mut TestBoolWriter,
        probabilities: &CoefficientProbabilities,
        coefficient_type: CoefficientBlockType,
        position: usize,
        context: usize,
    ) {
        writer.write_bool(
            false,
            coefficient_nodes(probabilities, coefficient_type, position, context)[0],
        );
    }

    fn key_frame(
        width: u16,
        height: u16,
        version: u8,
        show_frame: bool,
        partition_len: u32,
    ) -> [u8; KEY_FRAME_HEADER_LEN] {
        let tag = (partition_len << 5) | (u32::from(show_frame) << 4) | (u32::from(version) << 1);
        let mut payload = [0_u8; KEY_FRAME_HEADER_LEN];
        payload[..3].copy_from_slice(&tag.to_le_bytes()[..3]);
        payload[3..6].copy_from_slice(&KEY_FRAME_START_CODE);
        payload[6..8].copy_from_slice(&width.to_le_bytes());
        payload[8..10].copy_from_slice(&height.to_le_bytes());
        payload
    }

    #[test]
    fn parses_key_frame_dimensions_tag_and_scale_bits() {
        let payload = key_frame(0x800d, 0xc009, 3, true, 0);
        let header = parse_riff_payload(&payload, Some((13, 9)), &DecodeLimits::default()).unwrap();
        assert_eq!(header.width, 13);
        assert_eq!(header.height, 9);
        assert_eq!(header.version, 3);
        assert_eq!(header.first_partition_len, 0);
        assert_eq!(header.horizontal_scale, 2);
        assert_eq!(header.vertical_scale, 3);
    }

    #[test]
    fn rejects_all_fixed_header_truncations() {
        let payload = key_frame(1, 1, 0, true, 0);
        for end in 0..KEY_FRAME_HEADER_LEN {
            assert_eq!(
                parse_riff_payload(&payload[..end], None, &DecodeLimits::default())
                    .unwrap_err()
                    .kind(),
                DecodeErrorKind::UnexpectedEof,
                "truncation at {end}",
            );
        }
    }

    #[test]
    fn rejects_invalid_tag_signature_dimensions_partition_and_canvas() {
        let limits = DecodeLimits::default();
        let mut inter = key_frame(1, 1, 0, true, 0);
        inter[0] |= 1;
        assert_eq!(
            parse_riff_payload(&inter, None, &limits)
                .unwrap_err()
                .kind(),
            DecodeErrorKind::UnsupportedFeature
        );

        let invisible = key_frame(1, 1, 0, false, 0);
        assert_eq!(
            parse_riff_payload(&invisible, None, &limits)
                .unwrap_err()
                .kind(),
            DecodeErrorKind::InvalidBitstream
        );

        let unsupported_version = key_frame(1, 1, 4, true, 0);
        assert_eq!(
            parse_riff_payload(&unsupported_version, None, &limits)
                .unwrap_err()
                .kind(),
            DecodeErrorKind::InvalidBitstream
        );

        let mut bad_signature = key_frame(1, 1, 0, true, 0);
        bad_signature[5] ^= 1;
        assert_eq!(
            parse_riff_payload(&bad_signature, None, &limits)
                .unwrap_err()
                .kind(),
            DecodeErrorKind::InvalidBitstream
        );

        let zero_width = key_frame(0, 1, 0, true, 0);
        assert_eq!(
            parse_riff_payload(&zero_width, None, &limits)
                .unwrap_err()
                .kind(),
            DecodeErrorKind::InvalidBitstream
        );

        let partition_past_end = key_frame(1, 1, 0, true, 1);
        assert_eq!(
            parse_riff_payload(&partition_past_end, None, &limits)
                .unwrap_err()
                .kind(),
            DecodeErrorKind::UnexpectedEof
        );

        let valid = key_frame(1, 1, 0, true, 0);
        assert_eq!(
            parse_riff_payload(&valid, Some((2, 1)), &limits)
                .unwrap_err()
                .kind(),
            DecodeErrorKind::InvalidContainer
        );
    }

    #[test]
    fn enforces_image_limits_before_decoder_state_is_created() {
        let payload = key_frame(8, 1, 0, true, 0);
        let limits = DecodeLimits {
            max_width: 7,
            ..DecodeLimits::default()
        };
        assert_eq!(
            parse_riff_payload(&payload, None, &limits)
                .unwrap_err()
                .kind(),
            DecodeErrorKind::LimitExceeded
        );
    }

    #[test]
    fn parses_first_partition_controls_and_four_token_partitions() {
        let mut writer = TestBoolWriter::new();
        writer.write_bool(false, 128); // colour space
        writer.write_bool(true, 128); // clamp type
        writer.write_bool(true, 128); // segmentation enabled
        writer.write_bool(true, 128); // update segment map
        writer.write_bool(true, 128); // update segment data
        writer.write_bool(false, 128); // delta rather than absolute values
        for value in [-5, 0, 3, 0] {
            writer.write_bool(value != 0, 128);
            if value != 0 {
                writer.write_signed_literal(value, 7);
            }
        }
        for value in [-4, 0, 0, 0] {
            writer.write_bool(value != 0, 128);
            if value != 0 {
                writer.write_signed_literal(value, 6);
            }
        }
        for value in [11_u8, 255, 77] {
            writer.write_bool(value != 255, 128);
            if value != 255 {
                writer.write_literal(u32::from(value), 8);
            }
        }
        writer.write_bool(false, 128); // normal filter
        writer.write_literal(17, 6);
        writer.write_literal(4, 3);
        writer.write_bool(true, 128); // loop-filter deltas enabled
        writer.write_bool(true, 128); // update deltas
        for value in [2, 0, 0, 0] {
            writer.write_bool(value != 0, 128);
            if value != 0 {
                writer.write_signed_literal(value, 6);
            }
        }
        for value in [0, 0, 0, -1] {
            writer.write_bool(value != 0, 128);
            if value != 0 {
                writer.write_signed_literal(value, 6);
            }
        }
        writer.write_literal(2, 2); // four coefficient-token partitions
        write_quantization_header(&mut writer, 63, [-7, 0, 4, 0, -3], false);
        write_coefficient_updates(&mut writer, &[], false, 0);
        pad_first_partition(&mut writer);
        let mut partition_zero = writer.finish();
        partition_zero.extend_from_slice(&[0; 8]);

        let mut payload = key_frame(3, 5, 0, true, partition_zero.len() as u32).to_vec();
        payload.extend_from_slice(&partition_zero);
        payload.extend_from_slice(&[1, 0, 0, 2, 0, 0, 0, 0, 0]);
        payload.extend_from_slice(&[0xaa, 0xbb, 0xcc, 0xdd]);
        let frame = parse_riff_payload(&payload, None, &DecodeLimits::default()).unwrap();
        let layout = parse_partition_layout(&payload, &frame, &DecodeLimits::default()).unwrap();

        assert!(!layout.header.colorspace_reserved);
        assert!(layout.header.clamp_type);
        assert_eq!(layout.header.token_partition_count, 4);
        assert_eq!(layout.header.segments.quantizer, [-5, 0, 3, 0]);
        assert_eq!(layout.header.segments.filter_strength, [-4, 0, 0, 0]);
        assert_eq!(layout.header.segments.probabilities, [11, 255, 77]);
        assert_eq!(layout.header.filter.level, 17);
        assert_eq!(layout.header.filter.sharpness, 4);
        assert_eq!(layout.header.filter.ref_deltas, [2, 0, 0, 0]);
        assert_eq!(layout.header.filter.mode_deltas, [0, 0, 0, -1]);
        assert_eq!(
            layout.header.quantization,
            QuantizationHeader {
                base_index: 63,
                y1_dc_delta: -7,
                y2_dc_delta: 0,
                y2_ac_delta: 4,
                uv_dc_delta: 0,
                uv_ac_delta: -3,
            }
        );
        assert!(!layout.header.refresh_entropy_probabilities);
        assert_eq!(layout.header.coefficients.get(0, 0, 0, 0), 128);
        assert_eq!(layout.header.coefficients.get(0, 1, 0, 0), 253);
        assert_eq!(layout.header.coefficients.get(3, 7, 2, 10), 128);
        assert!(!layout.header.coefficients.use_skip_probability);
        assert_eq!(layout.header.coefficients.skip_probability, 0);
        assert_eq!(
            layout
                .tokens
                .iter()
                .map(|part| part.data)
                .collect::<Vec<_>>(),
            vec![&[0xaa][..], &[0xbb, 0xcc], &[], &[0xdd]],
        );
    }

    #[test]
    fn rejects_truncated_or_oversized_token_partition_tables() {
        let mut writer = TestBoolWriter::new();
        writer.write_bool(false, 128); // colour space
        writer.write_bool(false, 128); // clamp type
        writer.write_bool(false, 128); // no segmentation
        writer.write_bool(false, 128); // normal filter
        writer.write_literal(0, 6);
        writer.write_literal(0, 3);
        writer.write_bool(false, 128); // no filter deltas
        writer.write_literal(2, 2); // four token partitions
        write_quantization_header(&mut writer, 0, [0; 5], false);
        write_coefficient_updates(&mut writer, &[], false, 0);
        pad_first_partition(&mut writer);
        let partition_zero = writer.finish();
        let mut payload = key_frame(1, 1, 0, true, partition_zero.len() as u32).to_vec();
        payload.extend_from_slice(&partition_zero);
        let frame = parse_riff_payload(&payload, None, &DecodeLimits::default()).unwrap();
        assert_eq!(
            parse_partition_layout(&payload, &frame, &DecodeLimits::default())
                .unwrap_err()
                .kind(),
            DecodeErrorKind::UnexpectedEof
        );

        payload.extend_from_slice(&[5, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(
            parse_partition_layout(&payload, &frame, &DecodeLimits::default())
                .unwrap_err()
                .kind(),
            DecodeErrorKind::UnexpectedEof
        );
    }

    #[test]
    fn parses_each_legal_token_partition_count() {
        for partition_bits in 0..4_u32 {
            let mut writer = TestBoolWriter::new();
            writer.write_bool(false, 128); // colour space
            writer.write_bool(false, 128); // clamp type
            writer.write_bool(false, 128); // no segmentation
            writer.write_bool(false, 128); // normal filter
            writer.write_literal(0, 6);
            writer.write_literal(0, 3);
            writer.write_bool(false, 128); // no filter deltas
            writer.write_literal(partition_bits, 2);
            write_quantization_header(&mut writer, 0, [0; 5], false);
            write_coefficient_updates(&mut writer, &[], false, 0);
            pad_first_partition(&mut writer);
            let partition_zero = writer.finish();
            let partition_count = 1_usize << partition_bits;
            let mut payload = key_frame(1, 1, 0, true, partition_zero.len() as u32).to_vec();
            payload.extend_from_slice(&partition_zero);
            payload.resize(payload.len() + 3 * (partition_count - 1), 0);
            payload.push(0);

            let frame = parse_riff_payload(&payload, None, &DecodeLimits::default()).unwrap();
            let layout =
                parse_partition_layout(&payload, &frame, &DecodeLimits::default()).unwrap();
            assert_eq!(
                layout.header.token_partition_count as usize,
                partition_count
            );
            assert_eq!(layout.tokens.len(), partition_count);
            assert_eq!(layout.tokens.last().unwrap().data, &[0]);
        }
    }

    #[test]
    fn inverse_dct_preserves_zero_and_dc_microvectors() {
        assert_eq!(inverse_dct_4x4([0; 16]), [0; 16]);
        let mut dc = [0_i16; 16];
        dc[0] = 16;
        assert_eq!(inverse_dct_4x4(dc), [2; 16]);
    }

    #[test]
    fn inverse_wht_distributes_y2_dc_to_all_macroblock_blocks() {
        assert_eq!(inverse_wht_4x4([0; 16]), [0; 16]);
        let mut dc = [0_i16; 16];
        dc[0] = 8;
        assert_eq!(inverse_wht_4x4(dc), [1; 16]);
    }

    #[test]
    fn widened_transforms_and_macroblock_dequantization_preserve_y2_dc_layout() {
        let mut dc = [0_i32; 16];
        dc[0] = 16;
        assert_eq!(inverse_dct_4x4_i32(dc), [2; 16]);

        let empty = DecodedCoefficients {
            values: [0; 16],
            end: 0,
            non_zero: 0,
        };
        let mut residuals = MacroblockResiduals {
            y2: Some(DecodedCoefficients {
                values: [8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
                end: 1,
                non_zero: 1,
            }),
            luma: [empty; 16],
            u: [empty; 4],
            v: [empty; 4],
            non_zero_y: 0,
            non_zero_uv: 0,
        };
        residuals.luma[0].values[1] = 2;
        residuals.u[0].values[0] = 3;
        residuals.u[0].values[1] = -2;
        let matrix = DequantizationMatrix {
            y1_dc: 2,
            y1_ac: 3,
            y2_dc: 4,
            y2_ac: 5,
            uv_dc: 6,
            uv_ac: 7,
            uv_quant: 0,
        };

        let dequantized = dequantize_macroblock(&residuals, matrix);
        assert_eq!(dequantized.luma[0][0], 4);
        assert_eq!(dequantized.luma[15][0], 4);
        assert_eq!(dequantized.luma[0][1], 6);
        assert_eq!(dequantized.u[0][0], 18);
        assert_eq!(dequantized.u[0][1], -14);
        let spatial = inverse_transform_macroblock(dequantized);
        assert_eq!(spatial.luma[0], inverse_dct_4x4_i32(dequantized.luma[0]));
    }

    #[test]
    fn macroblock_sample_composition_maps_blocks_and_clips_edges() {
        let mut residues = MacroblockSpatialResidues {
            luma: [[0; 16]; 16],
            u: [[0; 16]; 4],
            v: [[0; 16]; 4],
        };
        residues.luma[0][0] = 2;
        residues.luma[5][6] = -3;
        residues.u[3][15] = 200;
        residues.v[0][0] = -200;
        let pixels = combine_macroblock_prediction(
            MacroblockPixels {
                y: [128; 256],
                u: [128; 64],
                v: [128; 64],
            },
            residues,
        );
        assert_eq!(pixels.y[0], 130);
        assert_eq!(pixels.y[5 * 16 + 6], 125);
        assert_eq!(pixels.u[7 * 8 + 7], 255);
        assert_eq!(pixels.v[0], 0);
        assert_eq!(add_residue_and_clip(0, -1), 0);
        assert_eq!(add_residue_and_clip(255, 1), 255);
    }

    #[test]
    fn intra16_prediction_uses_neighbours_and_dc_boundary_fallbacks() {
        let edges = MacroblockPredictionEdges {
            top_y: Some([10; 16]),
            top_right_y: Some([10; 4]),
            left_y: Some([30; 16]),
            top_left_y: 5,
            top_u: Some([50; 8]),
            left_u: Some([70; 8]),
            top_left_u: 20,
            top_v: Some([80; 8]),
            left_v: Some([90; 8]),
            top_left_v: 30,
        };
        let prediction =
            predict_intra16_macroblock(Intra16Mode::Vertical, ChromaMode::Horizontal, edges);
        assert_eq!(prediction.y, [10; 256]);
        assert_eq!(prediction.u, [70; 64]);
        assert_eq!(prediction.v, [90; 64]);

        let true_motion =
            predict_intra16_macroblock(Intra16Mode::TrueMotion, ChromaMode::TrueMotion, edges);
        assert_eq!(true_motion.y, [35; 256]);
        assert_eq!(true_motion.u, [100; 64]);
        assert_eq!(true_motion.v, [140; 64]);

        let dc = predict_intra16_macroblock(
            Intra16Mode::Dc,
            ChromaMode::Dc,
            MacroblockPredictionEdges::default(),
        );
        assert_eq!(dc.y, [128; 256]);
        assert_eq!(dc.u, [128; 64]);
        assert_eq!(dc.v, [128; 64]);
        let sentinel = predict_intra16_macroblock(
            Intra16Mode::Vertical,
            ChromaMode::Horizontal,
            MacroblockPredictionEdges::default(),
        );
        assert_eq!(sentinel.y, [127; 256]);
        assert_eq!(sentinel.u, [129; 64]);
    }

    #[test]
    fn intra4_prediction_covers_all_vp8_directional_modes() {
        let top = [10, 20, 30, 40, 50, 60, 70, 80];
        let left = [50, 60, 70, 80];
        let dc = predict_intra4_block(Intra4Mode::Dc, 5, top, left);
        assert_eq!(dc, [45; 16]);
        let true_motion = predict_intra4_block(Intra4Mode::TrueMotion, 5, top, left);
        assert_eq!(true_motion[0], 55);
        assert_eq!(true_motion[15], 115);
        assert_eq!(
            predict_intra4_block(Intra4Mode::Vertical, 5, top, left),
            [
                11, 20, 30, 40, 11, 20, 30, 40, 11, 20, 30, 40, 11, 20, 30, 40
            ]
        );
        assert_eq!(
            predict_intra4_block(Intra4Mode::Horizontal, 5, top, left),
            [
                41, 41, 41, 41, 60, 60, 60, 60, 70, 70, 70, 70, 78, 78, 78, 78
            ]
        );
        for mode in [
            Intra4Mode::DiagonalDownRight,
            Intra4Mode::VerticalRight,
            Intra4Mode::DiagonalDownLeft,
            Intra4Mode::VerticalLeft,
            Intra4Mode::HorizontalDown,
            Intra4Mode::HorizontalUp,
        ] {
            let prediction = predict_intra4_block(mode, 5, top, left);
            assert_ne!(prediction, [128; 16], "{mode:?}");
        }
        let diagonal_left = predict_intra4_block(Intra4Mode::DiagonalDownLeft, 5, top, left);
        assert_eq!(diagonal_left[0], 20);
        assert_eq!(diagonal_left[15], 78);
        let horizontal_up = predict_intra4_block(Intra4Mode::HorizontalUp, 5, top, left);
        assert_eq!(horizontal_up[12..], [80; 4]);
    }

    #[test]
    fn intra4_macroblock_and_full_reconstruction_follow_raster_neighbours() {
        let edges = MacroblockPredictionEdges {
            top_y: Some([10; 16]),
            top_right_y: Some([10; 4]),
            left_y: Some([30; 16]),
            top_left_y: 5,
            ..MacroblockPredictionEdges::default()
        };
        let prediction = predict_intra4_macroblock([Intra4Mode::Dc; 16], edges);
        assert_eq!(prediction[0], 20);
        assert_eq!(prediction[4], 15);

        let empty = DecodedCoefficients {
            values: [0; 16],
            end: 0,
            non_zero: 0,
        };
        let residuals = MacroblockResiduals {
            y2: None,
            luma: [empty; 16],
            u: [empty; 4],
            v: [empty; 4],
            non_zero_y: 0,
            non_zero_uv: 0,
        };
        let pixels = reconstruct_intra_macroblock(
            IntraMacroblock {
                segment: 0,
                skip: true,
                luma: LumaMode::FourByFour([Intra4Mode::Dc; 16]),
                chroma: ChromaMode::Dc,
            },
            &residuals,
            DequantizationMatrix {
                y1_dc: 1,
                y1_ac: 1,
                y2_dc: 1,
                y2_ac: 1,
                uv_dc: 1,
                uv_ac: 1,
                uv_quant: 0,
            },
            MacroblockPredictionEdges::default(),
        )
        .unwrap();
        assert!(pixels.y[..64].iter().all(|&value| value == 128));
        assert!(pixels.y[64..].iter().all(|&value| value == 129));
        assert_eq!(pixels.u, [128; 64]);
        assert_eq!(pixels.v, [128; 64]);
    }

    #[test]
    fn intra4_reconstruction_uses_residue_adjusted_left_neighbour() {
        let empty = DecodedCoefficients {
            values: [0; 16],
            end: 0,
            non_zero: 0,
        };
        let mut residuals = MacroblockResiduals {
            y2: None,
            luma: [empty; 16],
            u: [empty; 4],
            v: [empty; 4],
            non_zero_y: 0,
            non_zero_uv: 0,
        };
        residuals.luma[0].values[0] = 160;
        let pixels = reconstruct_intra_macroblock(
            IntraMacroblock {
                segment: 0,
                skip: false,
                luma: LumaMode::FourByFour([Intra4Mode::Horizontal; 16]),
                chroma: ChromaMode::Dc,
            },
            &residuals,
            DequantizationMatrix {
                y1_dc: 1,
                y1_ac: 1,
                y2_dc: 1,
                y2_ac: 1,
                uv_dc: 1,
                uv_ac: 1,
                uv_quant: 0,
            },
            MacroblockPredictionEdges::default(),
        )
        .unwrap();
        assert!(pixels.y[0] > 129);
        assert!(pixels.y[4] > 129);
    }

    #[test]
    fn coefficient_decoder_handles_eob_zero_runs_signs_and_zigzag() {
        let probabilities = CoefficientProbabilities::default();
        let mut writer = TestBoolWriter::new();

        let initial = coefficient_nodes(&probabilities, CoefficientBlockType::Luma16Ac, 0, 0);
        writer.write_bool(true, initial[0]); // not EOB
        writer.write_bool(false, initial[1]); // zero at position zero
        let position_one = coefficient_nodes(&probabilities, CoefficientBlockType::Luma16Ac, 1, 0);
        writer.write_bool(false, position_one[1]); // zero at position one
        let position_two = coefficient_nodes(&probabilities, CoefficientBlockType::Luma16Ac, 2, 0);
        writer.write_bool(true, position_two[1]);
        writer.write_bool(false, position_two[2]); // magnitude one
        writer.write_bool(true, 128); // negative
        let next = coefficient_nodes(&probabilities, CoefficientBlockType::Luma16Ac, 3, 1);
        writer.write_bool(false, next[0]); // EOB

        let bytes = writer.finish();
        let mut decoder = BoolDecoder::new(&bytes, &DecodeLimits::default()).unwrap();
        let decoded = decode_coefficients(
            &mut decoder,
            &probabilities,
            CoefficientBlockType::Luma16Ac,
            0,
            0,
        )
        .unwrap();
        let mut expected = [0_i16; 16];
        expected[COEFFICIENT_ZIGZAG[2]] = -1;
        assert_eq!(decoded.values, expected);
        assert_eq!(decoded.end, 3);
        assert_eq!(decoded.non_zero, 1);
    }

    #[test]
    fn coefficient_decoder_handles_large_category_values_and_ac_only_start() {
        let probabilities = CoefficientProbabilities::default();
        let mut writer = TestBoolWriter::new();

        let nodes = coefficient_nodes(&probabilities, CoefficientBlockType::Luma4Ac, 1, 2);
        writer.write_bool(true, nodes[0]); // not EOB
        writer.write_bool(true, nodes[1]); // non-zero
        writer.write_bool(true, nodes[2]); // value exceeds one
        writer.write_bool(true, nodes[3]); // category path
        writer.write_bool(true, nodes[6]);
        writer.write_bool(false, nodes[8]);
        writer.write_bool(true, nodes[9]);
        for &probability in CATEGORY_PROBABILITIES[2] {
            writer.write_bool(false, probability);
        }
        writer.write_bool(false, 128); // positive sign
        let next = coefficient_nodes(&probabilities, CoefficientBlockType::Luma4Ac, 2, 2);
        writer.write_bool(false, next[0]); // EOB

        let bytes = writer.finish();
        let mut decoder = BoolDecoder::new(&bytes, &DecodeLimits::default()).unwrap();
        let decoded = decode_coefficients(
            &mut decoder,
            &probabilities,
            CoefficientBlockType::Luma4Ac,
            2,
            1,
        )
        .unwrap();
        let mut expected = [0_i16; 16];
        // This uses the category-five branch (base magnitude 35) with a zero
        // suffix, exercising the longest category tree selected by this
        // compact vector.
        expected[COEFFICIENT_ZIGZAG[1]] = 35;
        assert_eq!(decoded.values, expected);
        assert_eq!(decoded.end, 2);
        assert_eq!(decoded.non_zero, 1);
    }

    #[test]
    fn coefficient_decoder_rejects_invalid_context_and_start() {
        let probabilities = CoefficientProbabilities::default();
        let mut decoder = BoolDecoder::new(&[0], &DecodeLimits::default()).unwrap();
        assert_eq!(
            decode_coefficients(
                &mut decoder,
                &probabilities,
                CoefficientBlockType::LumaDc,
                3,
                0,
            )
            .unwrap_err()
            .kind(),
            DecodeErrorKind::InvalidParameter
        );
        assert_eq!(
            decode_coefficients(
                &mut decoder,
                &probabilities,
                CoefficientBlockType::LumaDc,
                0,
                16,
            )
            .unwrap_err()
            .kind(),
            DecodeErrorKind::InvalidParameter
        );
    }

    #[test]
    fn intra_frame_decoder_reconstructs_a_skipped_macroblock() {
        let mut writer = TestBoolWriter::new();
        writer.write_bool(false, 128); // colour space
        writer.write_bool(false, 128); // clamp type
        writer.write_bool(false, 128); // no segmentation
        writer.write_bool(false, 128); // normal filter
        writer.write_literal(0, 6); // filter level
        writer.write_literal(0, 3); // filter sharpness
        writer.write_bool(false, 128); // no filter deltas
        writer.write_literal(0, 2); // one token partition
        write_quantization_header(&mut writer, 0, [0; 5], false);
        write_coefficient_updates(&mut writer, &[], true, 1);
        writer.write_bool(true, 1); // skip residuals
        writer.write_bool(true, 145); // 16x16 luma
        writer.write_bool(false, 156); // DC luma
        writer.write_bool(false, 163);
        writer.write_bool(false, 142); // DC chroma
        let partition_zero = writer.finish();
        let mut payload = key_frame(1, 1, 0, true, partition_zero.len() as u32).to_vec();
        payload.extend_from_slice(&partition_zero);
        payload.push(0); // Non-empty final token partition, never consumed by skip.

        let limits = DecodeLimits::default();
        let frame = parse_riff_payload(&payload, None, &limits).unwrap();
        let image = decode_intra_frame(&payload, &frame, &limits).unwrap();
        assert_eq!(image.width, 1);
        assert_eq!(image.height, 1);
        assert_eq!(image.y_stride, 16);
        assert_eq!(image.uv_stride, 8);
        assert_eq!(image.y.len(), 16 * 16);
        assert_eq!(image.u.len(), 8 * 8);
        assert!(image.y.iter().all(|&sample| sample == 128));
        assert!(image.u.iter().all(|&sample| sample == 128));
        assert!(image.v.iter().all(|&sample| sample == 128));
    }

    #[test]
    fn macroblock_storage_exposes_reconstructed_edges() {
        let frame = Vp8Header {
            width: 17,
            height: 17,
            version: 0,
            first_partition_len: 0,
            horizontal_scale: 0,
            vertical_scale: 0,
        };
        let mut image = Vp8YuvImage::new(&frame, &DecodeLimits::default()).unwrap();
        let pixels = MacroblockPixels {
            y: std::array::from_fn(|index| index as u8),
            u: std::array::from_fn(|index| (index + 64) as u8),
            v: std::array::from_fn(|index| (index + 128) as u8),
        };
        image.store_macroblock(0, 0, pixels);
        let right_edges = image.edges(1, 0);
        assert_eq!(
            right_edges.left_y.unwrap(),
            std::array::from_fn(|row| (row * 16 + 15) as u8)
        );
        assert_eq!(
            right_edges.left_u.unwrap(),
            std::array::from_fn(|row| (row * 8 + 71) as u8)
        );
        assert_eq!(
            right_edges.left_v.unwrap(),
            std::array::from_fn(|row| (row * 8 + 135) as u8)
        );
        let below_edges = image.edges(0, 1);
        assert_eq!(below_edges.top_y.unwrap(), pixels.y[240..256]);
        assert_eq!(below_edges.top_u.unwrap(), pixels.u[56..64]);
        assert_eq!(below_edges.top_v.unwrap(), pixels.v[56..64]);
    }

    #[test]
    fn macroblock_storage_enforces_allocation_limit() {
        let frame = Vp8Header {
            width: 1,
            height: 1,
            version: 0,
            first_partition_len: 0,
            horizontal_scale: 0,
            vertical_scale: 0,
        };
        let limits = DecodeLimits {
            max_alloc_bytes: 383,
            ..DecodeLimits::default()
        };
        assert_eq!(
            Vp8YuvImage::new(&frame, &limits).unwrap_err().kind(),
            DecodeErrorKind::LimitExceeded
        );
    }

    #[test]
    fn yuv_image_converts_visible_rectangle_to_vp8_rgba() {
        let image = Vp8YuvImage {
            width: 2,
            height: 2,
            y_stride: 2,
            uv_stride: 1,
            y: vec![16, 235, 81, 145],
            u: vec![128],
            v: vec![128],
        };
        assert_eq!(
            image.to_rgba(&DecodeLimits::default()).unwrap(),
            vec![
                0, 0, 0, 255, 255, 255, 255, 255, 76, 76, 76, 255, 150, 150, 150, 255
            ]
        );
    }

    #[test]
    fn yuv_image_rejects_short_visible_plane() {
        let image = Vp8YuvImage {
            width: 2,
            height: 2,
            y_stride: 2,
            uv_stride: 1,
            y: vec![0; 3],
            u: vec![128],
            v: vec![128],
        };
        assert_eq!(
            image.to_rgba(&DecodeLimits::default()).unwrap_err().kind(),
            DecodeErrorKind::InvalidParameter
        );
    }

    #[test]
    fn residual_decoder_consumes_all_intra_block_families_and_preserves_empty_contexts() {
        let probabilities = CoefficientProbabilities::default();
        let mut writer = TestBoolWriter::new();
        write_coefficient_eob(
            &mut writer,
            &probabilities,
            CoefficientBlockType::LumaDc,
            0,
            0,
        );
        for _ in 0..16 {
            write_coefficient_eob(
                &mut writer,
                &probabilities,
                CoefficientBlockType::Luma16Ac,
                1,
                0,
            );
        }
        for _ in 0..8 {
            write_coefficient_eob(
                &mut writer,
                &probabilities,
                CoefficientBlockType::ChromaAc,
                0,
                0,
            );
        }

        let bytes = writer.finish();
        let mut decoder = BoolDecoder::new(&bytes, &DecodeLimits::default()).unwrap();
        let mut top = ResidualContext::default();
        let mut left = ResidualContext::default();
        let residuals =
            decode_intra_residuals(&mut decoder, &probabilities, false, &mut top, &mut left)
                .unwrap();
        assert_eq!(residuals.y2.unwrap().end, 0);
        assert!(residuals.luma.iter().all(|block| block.non_zero == 0));
        assert!(residuals.u.iter().all(|block| block.non_zero == 0));
        assert!(residuals.v.iter().all(|block| block.non_zero == 0));
        assert_eq!(residuals.non_zero_y, 0);
        assert_eq!(residuals.non_zero_uv, 0);
        assert_eq!(top, ResidualContext::default());
        assert_eq!(left, ResidualContext::default());
    }
}
