//! Suspendable VP8 macroblock-row reconstruction.

use crate::DecodeError;
use crate::DecodeLimits;
use crate::vp8::BoolDecoder;
use crate::vp8::BoolDecoderState;
use crate::vp8::ChromaMode;
use crate::vp8::DecodedCoefficients;
use crate::vp8::DequantizationMatrix;
use crate::vp8::IncrementalPartitionLayout;
use crate::vp8::Intra4Mode;
use crate::vp8::Intra16Mode;
use crate::vp8::IntraMacroblock;
use crate::vp8::LoopFilterStrength;
use crate::vp8::LumaMode;
use crate::vp8::MacroblockResiduals;
use crate::vp8::ResidualContext;
use crate::vp8::Vp8Header;
use crate::vp8::Vp8YuvImage;
use crate::vp8::decode_intra_residuals;
use crate::vp8::derive_dequantization;
use crate::vp8::derive_loop_filter_strengths;
use crate::vp8::loop_filter::MacroblockFilter;
use crate::vp8::loop_filter::filter_macroblock;
use crate::vp8::parse_incremental_partition_layout;
use crate::vp8::parse_intra_mode_row;
use crate::vp8::reconstruct_intra_macroblock;

#[derive(Clone, Debug)]
pub(crate) struct IncrementalVp8Decoder {
    frame: Vp8Header,
    layout: IncrementalPartitionLayout,
    mode_state: BoolDecoderState,
    token_states: Vec<BoolDecoderState>,
    image: Vp8YuvImage,
    matrices: [DequantizationMatrix; 4],
    strengths: [[LoopFilterStrength; 2]; 4],
    top_modes: Vec<Intra4Mode>,
    top_contexts: Vec<ResidualContext>,
    blocks: Vec<IntraMacroblock>,
    row_filters: Vec<(LoopFilterStrength, bool)>,
    row_prepared: bool,
    macroblock_x: usize,
    macroblock_y: usize,
    left_context: ResidualContext,
    decoded_rows: u32,
    rgba: Vec<u8>,
    alpha: Option<Vec<u8>>,
    limits: DecodeLimits,
}

impl IncrementalVp8Decoder {
    pub(crate) fn new(
        payload: &[u8],
        declared_payload_len: usize,
        frame: Vp8Header,
        alpha: Option<Vec<u8>>,
        limits: &DecodeLimits,
    ) -> Result<Self, DecodeError> {
        let layout =
            parse_incremental_partition_layout(payload, &frame, declared_payload_len, limits)?;
        let macroblock_width = macroblock_width(frame.width)?;
        let row_state_bytes = macroblock_width
            .checked_mul(4 * std::mem::size_of::<Intra4Mode>())
            .and_then(|bytes| {
                bytes.checked_add(macroblock_width * std::mem::size_of::<ResidualContext>())
            })
            .and_then(|bytes| {
                bytes.checked_add(macroblock_width * std::mem::size_of::<IntraMacroblock>())
            })
            .and_then(|bytes| {
                bytes.checked_add(
                    macroblock_width * std::mem::size_of::<(LoopFilterStrength, bool)>(),
                )
            })
            .ok_or_else(allocation_error)?;
        if row_state_bytes > limits.max_alloc_bytes {
            return Err(DecodeError::new(
                crate::DecodeErrorKind::LimitExceeded,
                None,
                "VP8 incremental row state exceeds configured allocation limit",
            ));
        }
        let image = Vp8YuvImage::new(&frame, limits)?;
        let token_states = filled_vec(
            BoolDecoderState::new(limits),
            layout.tokens.len(),
            "cannot allocate VP8 token decoder states",
        )?;
        let matrices = derive_dequantization(layout.header.quantization, &layout.header.segments);
        let strengths =
            derive_loop_filter_strengths(&layout.header.filter, &layout.header.segments);
        let mode_state = layout.mode_state;
        Ok(Self {
            frame,
            layout,
            mode_state,
            token_states,
            image,
            matrices,
            strengths,
            top_modes: filled_vec(
                Intra4Mode::Dc,
                macroblock_width * 4,
                "cannot allocate VP8 top prediction modes",
            )?,
            top_contexts: filled_vec(
                ResidualContext::default(),
                macroblock_width,
                "cannot allocate VP8 residual contexts",
            )?,
            blocks: filled_vec(
                empty_intra_macroblock(),
                macroblock_width,
                "cannot allocate VP8 macroblock row",
            )?,
            row_filters: filled_vec(
                (LoopFilterStrength::default(), false),
                macroblock_width,
                "cannot allocate VP8 row filter state",
            )?,
            row_prepared: false,
            macroblock_x: 0,
            macroblock_y: 0,
            left_context: ResidualContext::default(),
            decoded_rows: 0,
            rgba: Vec::new(),
            alpha,
            limits: limits.clone(),
        })
    }

    pub(crate) const fn width(&self) -> u32 {
        self.frame.width
    }

    pub(crate) const fn height(&self) -> u32 {
        self.frame.height
    }

    pub(crate) const fn decoded_rows(&self) -> u32 {
        self.decoded_rows
    }

    pub(crate) fn rgba(&self) -> &[u8] {
        &self.rgba
    }

    pub(crate) fn is_complete(&self) -> bool {
        self.decoded_rows == self.frame.height
    }

    pub(crate) fn into_rgba(self) -> Vec<u8> {
        self.rgba
    }

    /// Advances until either one token partition needs another byte or the
    /// final row is stable. A failed macroblock never commits arithmetic or
    /// neighbour context, matching libwebp's `SaveContext` boundary.
    pub(crate) fn advance(&mut self, payload: &[u8]) -> Result<(), DecodeError> {
        let macroblock_height = macroblock_height(self.frame.height)?;
        while self.macroblock_y < macroblock_height {
            if !self.row_prepared {
                let first = &payload[self.layout.first_partition.clone()];
                let mut mode_bits = BoolDecoder::from_state(first, self.mode_state);
                parse_intra_mode_row(
                    &mut mode_bits,
                    &self.layout.header,
                    &mut self.top_modes,
                    &mut self.blocks,
                )?;
                self.mode_state = mode_bits.state();
                self.left_context = ResidualContext::default();
                self.row_prepared = true;
            }

            while self.macroblock_x < self.blocks.len() {
                let x = self.macroblock_x;
                let block = self.blocks[x];
                let residuals = if block.skip {
                    self.top_contexts[x] = ResidualContext::default();
                    self.left_context = ResidualContext::default();
                    empty_macroblock_residuals()
                } else {
                    let partition = self.macroblock_y & (self.layout.tokens.len() - 1);
                    let data = available_partition(payload, &self.layout.tokens[partition]);
                    let mut token_bits =
                        BoolDecoder::from_state(data, self.token_states[partition]);
                    let saved_top = self.top_contexts[x];
                    let saved_left = self.left_context;
                    match decode_intra_residuals(
                        &mut token_bits,
                        &self.layout.header.coefficients,
                        matches!(block.luma, LumaMode::FourByFour(_)),
                        &mut self.top_contexts[x],
                        &mut self.left_context,
                    ) {
                        Ok(residuals) => {
                            self.token_states[partition] = token_bits.state();
                            residuals
                        }
                        Err(error) => {
                            self.top_contexts[x] = saved_top;
                            self.left_context = saved_left;
                            return Err(error);
                        }
                    }
                };
                let has_residuals = residuals.non_zero_y != 0 || residuals.non_zero_uv != 0;
                let matrix = *self
                    .matrices
                    .get(usize::from(block.segment))
                    .ok_or_else(|| {
                        DecodeError::new(
                            crate::DecodeErrorKind::InvalidBitstream,
                            None,
                            "VP8 macroblock segment exceeds four-entry quantizer table",
                        )
                    })?;
                let pixels = reconstruct_intra_macroblock(
                    block,
                    &residuals,
                    matrix,
                    self.image.edges(x, self.macroblock_y),
                )?;
                self.image.store_macroblock(x, self.macroblock_y, pixels);
                let strength = self
                    .strengths
                    .get(usize::from(block.segment))
                    .ok_or_else(|| {
                        DecodeError::new(
                            crate::DecodeErrorKind::InvalidBitstream,
                            None,
                            "VP8 macroblock segment exceeds four-entry filter table",
                        )
                    })?[usize::from(matches!(block.luma, LumaMode::FourByFour(_)))];
                self.row_filters[x] = (
                    strength,
                    strength.filters_inner(
                        matches!(block.luma, LumaMode::FourByFour(_)),
                        !has_residuals,
                    ),
                );
                self.macroblock_x += 1;
            }

            for (x, &(strength, filters_inner)) in self.row_filters.iter().enumerate() {
                filter_macroblock(MacroblockFilter {
                    y: &mut self.image.y,
                    u: &mut self.image.u,
                    v: &mut self.image.v,
                    y_stride: self.image.y_stride,
                    uv_stride: self.image.uv_stride,
                    macroblock_x: x,
                    macroblock_y: self.macroblock_y,
                    simple: self.layout.header.filter.simple,
                    strength,
                    filters_inner,
                });
            }
            self.macroblock_y += 1;
            self.macroblock_x = 0;
            self.row_prepared = false;
            let stable_rows = if self.macroblock_y == macroblock_height {
                self.frame.height
            } else {
                u32::try_from(self.macroblock_y.saturating_sub(1) * 16)
                    .unwrap_or(self.frame.height)
                    .min(self.frame.height)
            };
            self.append_stable_rows(stable_rows)?;
        }
        Ok(())
    }

    fn append_stable_rows(&mut self, stable_rows: u32) -> Result<(), DecodeError> {
        let old_len = self.rgba.len();
        self.image.append_rgba_rows(
            self.decoded_rows,
            stable_rows,
            &mut self.rgba,
            &self.limits,
        )?;
        if let Some(alpha) = &self.alpha {
            for (index, pixel) in self.rgba[old_len..].chunks_exact_mut(4).enumerate() {
                let row_start = usize::try_from(self.decoded_rows)
                    .ok()
                    .and_then(|row| row.checked_mul(usize::try_from(self.frame.width).ok()?))
                    .ok_or_else(allocation_error)?;
                pixel[3] = alpha[row_start + index];
            }
        }
        self.decoded_rows = stable_rows;
        Ok(())
    }
}

fn available_partition<'a>(payload: &'a [u8], range: &std::ops::Range<usize>) -> &'a [u8] {
    if payload.len() <= range.start {
        &payload[0..0]
    } else {
        &payload[range.start..payload.len().min(range.end)]
    }
}

fn macroblock_width(width: u32) -> Result<usize, DecodeError> {
    usize::try_from(width)
        .ok()
        .and_then(|width| width.checked_add(15))
        .map(|width| width / 16)
        .ok_or_else(allocation_error)
}

fn macroblock_height(height: u32) -> Result<usize, DecodeError> {
    usize::try_from(height)
        .ok()
        .and_then(|height| height.checked_add(15))
        .map(|height| height / 16)
        .ok_or_else(allocation_error)
}

fn allocation_error() -> DecodeError {
    DecodeError::new(
        crate::DecodeErrorKind::LimitExceeded,
        None,
        "VP8 incremental state size overflows",
    )
}

fn filled_vec<T: Clone>(
    value: T,
    len: usize,
    context: &'static str,
) -> Result<Vec<T>, DecodeError> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(len)
        .map_err(|_| DecodeError::new(crate::DecodeErrorKind::AllocationFailed, None, context))?;
    output.resize(len, value);
    Ok(output)
}

fn empty_intra_macroblock() -> IntraMacroblock {
    IntraMacroblock {
        segment: 0,
        skip: true,
        luma: LumaMode::Sixteen(Intra16Mode::Dc),
        chroma: ChromaMode::Dc,
    }
}

fn empty_macroblock_residuals() -> MacroblockResiduals {
    let empty = DecodedCoefficients {
        values: [0; 16],
        end: 0,
        non_zero: 0,
    };
    MacroblockResiduals {
        y2: None,
        luma: [empty; 16],
        u: [empty; 4],
        v: [empty; 4],
        non_zero_y: 0,
        non_zero_uv: 0,
    }
}
