//! VP8 frame reconstruction and macroblock-aligned YUV storage.

use webp_core::DecodeError;
use webp_core::DecodeErrorKind;
use webp_core::DecodeLimits;
use webp_core::checked_image_bytes;

use crate::BoolDecoder;
use crate::ChromaMode;
use crate::DecodedCoefficients;
use crate::Intra4Mode;
use crate::Intra16Mode;
use crate::IntraMacroblock;
use crate::LoopFilterStrength;
use crate::LumaMode;
use crate::MacroblockPixels;
use crate::MacroblockPredictionEdges;
use crate::MacroblockResiduals;
use crate::ResidualContext;
use crate::Vp8Header;
use crate::decode_intra_residuals;
use crate::derive_dequantization;
use crate::derive_loop_filter_strengths;
use crate::loop_filter::MacroblockFilter;
use crate::loop_filter::filter_macroblock;
use crate::parse_intra_mode_row;
use crate::partition::parse_partition_layout_with_mode_decoder;
use crate::reconstruct_intra_macroblock;

#[cfg(test)]
#[path = "frame_tests.rs"]
mod tests;

/// Macroblock-aligned YUV 4:2:0 samples reconstructed from a VP8 key frame.
///
/// `width` and `height` describe the visible picture. The plane strides and
/// lengths are rounded up to whole 16×16 macroblocks so the final partial
/// macroblock can retain its prediction border for subsequent decoding. Users
/// emitting pixels should copy only the visible rectangle from each plane.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Vp8YuvImage {
    pub width: u32,
    pub height: u32,
    pub y_stride: usize,
    pub uv_stride: usize,
    pub y: Vec<u8>,
    pub u: Vec<u8>,
    pub v: Vec<u8>,
}

impl Vp8YuvImage {
    /// Macroblock-aligned luma storage height.
    #[must_use]
    pub fn padded_height(&self) -> usize {
        self.y.len() / self.y_stride
    }

    /// Macroblock-aligned chroma storage height.
    #[must_use]
    pub fn padded_uv_height(&self) -> usize {
        self.u.len() / self.uv_stride
    }

    /// Converts the visible YUV 4:2:0 rectangle to opaque, straight RGBA8.
    ///
    /// Conversion uses VP8's fixed-point BT.601 coefficients and libwebp's
    /// fancy 4:2:0 chroma upsampling. Macroblock padding is never exposed in
    /// the returned buffer.
    pub fn to_rgba(&self, limits: &DecodeLimits) -> Result<Vec<u8>, DecodeError> {
        let width = usize::try_from(self.width).map_err(|_| allocation_size_error())?;
        let height = usize::try_from(self.height).map_err(|_| allocation_size_error())?;
        let uv_width = width.checked_add(1).ok_or_else(allocation_size_error)? / 2;
        let uv_height = height.checked_add(1).ok_or_else(allocation_size_error)? / 2;
        let y_required = self
            .y_stride
            .checked_mul(height)
            .ok_or_else(allocation_size_error)?;
        let uv_required = self
            .uv_stride
            .checked_mul(uv_height)
            .ok_or_else(allocation_size_error)?;
        if self.y_stride < width
            || self.uv_stride < uv_width
            || self.y.len() < y_required
            || self.u.len() < uv_required
            || self.v.len() < uv_required
        {
            return Err(DecodeError::new(
                DecodeErrorKind::InvalidParameter,
                None,
                "VP8 YUV planes do not contain the visible image rectangle",
            ));
        }
        let rgba_len = checked_image_bytes(self.width, self.height, 4)?;
        if rgba_len > limits.max_alloc_bytes {
            return Err(DecodeError::new(
                DecodeErrorKind::LimitExceeded,
                None,
                "VP8 RGBA output exceeds configured allocation limit",
            ));
        }
        let mut rgba = Vec::new();
        rgba.try_reserve_exact(rgba_len).map_err(|_| {
            DecodeError::new(
                DecodeErrorKind::AllocationFailed,
                None,
                "cannot allocate VP8 RGBA output",
            )
        })?;
        let chroma = ChromaSamplingGeometry {
            uv_width,
            uv_height,
            width,
            height,
        };
        for y in 0..height {
            let y_row = y * self.y_stride;
            for x in 0..width {
                let luma = self.y[y_row + x];
                let chroma_u = fancy_chroma_sample(&self.u, self.uv_stride, chroma, x, y);
                let chroma_v = fancy_chroma_sample(&self.v, self.uv_stride, chroma, x, y);
                let [red, green, blue] = vp8_yuv_to_rgb(luma, chroma_u, chroma_v);
                rgba.extend_from_slice(&[red, green, blue, 255]);
            }
        }
        Ok(rgba)
    }

    pub(crate) fn new(frame: &Vp8Header, limits: &DecodeLimits) -> Result<Self, DecodeError> {
        let (macroblock_width, macroblock_height) = macroblock_dimensions(frame)?;
        let y_stride = macroblock_width.checked_mul(16).ok_or_else(|| {
            DecodeError::new(
                DecodeErrorKind::LimitExceeded,
                None,
                "VP8 luma stride overflows",
            )
        })?;
        let padded_height = macroblock_height.checked_mul(16).ok_or_else(|| {
            DecodeError::new(
                DecodeErrorKind::LimitExceeded,
                None,
                "VP8 luma height overflows",
            )
        })?;
        let uv_stride = macroblock_width.checked_mul(8).ok_or_else(|| {
            DecodeError::new(
                DecodeErrorKind::LimitExceeded,
                None,
                "VP8 chroma stride overflows",
            )
        })?;
        let padded_uv_height = macroblock_height.checked_mul(8).ok_or_else(|| {
            DecodeError::new(
                DecodeErrorKind::LimitExceeded,
                None,
                "VP8 chroma height overflows",
            )
        })?;
        let y_len = checked_image_bytes(
            u32::try_from(y_stride).map_err(|_| allocation_size_error())?,
            u32::try_from(padded_height).map_err(|_| allocation_size_error())?,
            1,
        )?;
        let uv_len = checked_image_bytes(
            u32::try_from(uv_stride).map_err(|_| allocation_size_error())?,
            u32::try_from(padded_uv_height).map_err(|_| allocation_size_error())?,
            1,
        )?;
        let storage = y_len
            .checked_add(uv_len.checked_mul(2).ok_or_else(allocation_size_error)?)
            .ok_or_else(allocation_size_error)?;
        if storage > limits.max_alloc_bytes {
            return Err(DecodeError::new(
                DecodeErrorKind::LimitExceeded,
                None,
                "VP8 YUV frame storage exceeds configured allocation limit",
            ));
        }
        Ok(Self {
            width: frame.width,
            height: frame.height,
            y_stride,
            uv_stride,
            y: vec![0; y_len],
            u: vec![0; uv_len],
            v: vec![0; uv_len],
        })
    }

    pub(crate) fn edges(
        &self,
        macroblock_x: usize,
        macroblock_y: usize,
    ) -> MacroblockPredictionEdges {
        let y_origin = macroblock_y * 16 * self.y_stride + macroblock_x * 16;
        let uv_origin = macroblock_y * 8 * self.uv_stride + macroblock_x * 8;
        let top_y = (macroblock_y > 0)
            .then(|| std::array::from_fn(|index| self.y[y_origin - self.y_stride + index]));
        let top_right_y = (macroblock_y > 0 && macroblock_x + 1 < self.y_stride / 16)
            .then(|| std::array::from_fn(|index| self.y[y_origin - self.y_stride + 16 + index]));
        let left_y = (macroblock_x > 0)
            .then(|| std::array::from_fn(|index| self.y[y_origin + index * self.y_stride - 1]));
        let top_u = (macroblock_y > 0)
            .then(|| std::array::from_fn(|index| self.u[uv_origin - self.uv_stride + index]));
        let left_u = (macroblock_x > 0)
            .then(|| std::array::from_fn(|index| self.u[uv_origin + index * self.uv_stride - 1]));
        let top_v = (macroblock_y > 0)
            .then(|| std::array::from_fn(|index| self.v[uv_origin - self.uv_stride + index]));
        let left_v = (macroblock_x > 0)
            .then(|| std::array::from_fn(|index| self.v[uv_origin + index * self.uv_stride - 1]));
        MacroblockPredictionEdges {
            top_y,
            top_right_y,
            left_y,
            top_left_y: if macroblock_x > 0 && macroblock_y > 0 {
                self.y[y_origin - self.y_stride - 1]
            } else {
                0
            },
            top_u,
            left_u,
            top_left_u: if macroblock_x > 0 && macroblock_y > 0 {
                self.u[uv_origin - self.uv_stride - 1]
            } else {
                0
            },
            top_v,
            left_v,
            top_left_v: if macroblock_x > 0 && macroblock_y > 0 {
                self.v[uv_origin - self.uv_stride - 1]
            } else {
                0
            },
        }
    }

    pub(crate) fn store_macroblock(
        &mut self,
        macroblock_x: usize,
        macroblock_y: usize,
        pixels: MacroblockPixels,
    ) {
        let y_origin = macroblock_y * 16 * self.y_stride + macroblock_x * 16;
        let uv_origin = macroblock_y * 8 * self.uv_stride + macroblock_x * 8;
        for row in 0..16 {
            self.y[y_origin + row * self.y_stride..y_origin + row * self.y_stride + 16]
                .copy_from_slice(&pixels.y[row * 16..row * 16 + 16]);
        }
        for row in 0..8 {
            self.u[uv_origin + row * self.uv_stride..uv_origin + row * self.uv_stride + 8]
                .copy_from_slice(&pixels.u[row * 8..row * 8 + 8]);
            self.v[uv_origin + row * self.uv_stride..uv_origin + row * self.uv_stride + 8]
                .copy_from_slice(&pixels.v[row * 8..row * 8 + 8]);
        }
    }
}

/// Decodes all intra macroblocks of a WebP VP8 key frame into YUV samples.
///
/// The VP8 payload and [`Vp8Header`] must originate from the same RIFF chunk.
/// Mode data is consumed in raster order from partition zero; coefficient
/// tokens select their partition using `macroblock_row & (count - 1)`, exactly
/// as specified by VP8. Each reconstructed row is loop-filtered before the
/// next row consumes it as an intra-prediction boundary.
pub fn decode_intra_frame(
    payload: &[u8],
    frame: &Vp8Header,
    limits: &DecodeLimits,
) -> Result<Vp8YuvImage, DecodeError> {
    limits.check_image(frame.width, frame.height)?;
    let (layout, mut mode_bits) = parse_partition_layout_with_mode_decoder(payload, frame, limits)?;
    let mut token_bits = layout
        .tokens
        .iter()
        .map(|partition| BoolDecoder::new(partition.data, limits))
        .collect::<Result<Vec<_>, _>>()?;
    let (macroblock_width, macroblock_height) = macroblock_dimensions(frame)?;
    let context_bytes = macroblock_width
        .checked_mul(4 * std::mem::size_of::<Intra4Mode>())
        .and_then(|size| {
            size.checked_add(macroblock_width * std::mem::size_of::<ResidualContext>())
        })
        .and_then(|size| {
            size.checked_add(macroblock_width * std::mem::size_of::<IntraMacroblock>())
        })
        .ok_or_else(allocation_size_error)?;
    if context_bytes > limits.max_alloc_bytes {
        return Err(DecodeError::new(
            DecodeErrorKind::LimitExceeded,
            None,
            "VP8 macroblock row state exceeds configured allocation limit",
        ));
    }
    let mut image = Vp8YuvImage::new(frame, limits)?;
    let matrices = derive_dequantization(layout.header.quantization, &layout.header.segments);
    let mut top_modes = vec![Intra4Mode::Dc; macroblock_width * 4];
    let mut top_contexts = vec![ResidualContext::default(); macroblock_width];
    let mut blocks = vec![empty_intra_macroblock(); macroblock_width];
    let strengths = derive_loop_filter_strengths(&layout.header.filter, &layout.header.segments);
    let mut row_filters = vec![(LoopFilterStrength::default(), false); macroblock_width];
    for macroblock_y in 0..macroblock_height {
        parse_intra_mode_row(&mut mode_bits, &layout.header, &mut top_modes, &mut blocks)?;
        let mut left_context = ResidualContext::default();
        let partition = macroblock_y & (token_bits.len() - 1);
        for (macroblock_x, &block) in blocks.iter().enumerate() {
            let residuals = if block.skip {
                top_contexts[macroblock_x] = ResidualContext::default();
                left_context = ResidualContext::default();
                empty_macroblock_residuals()
            } else {
                decode_intra_residuals(
                    &mut token_bits[partition],
                    &layout.header.coefficients,
                    matches!(block.luma, LumaMode::FourByFour(_)),
                    &mut top_contexts[macroblock_x],
                    &mut left_context,
                )?
            };
            // libwebp's loop filter treats a macroblock with no decoded
            // residuals like a skipped macroblock, even when the optional
            // skip bit was not present. Its internal 4×4 edges must therefore
            // remain unfiltered unless the macroblock is B_PRED.
            let has_residuals = residuals.non_zero_y != 0 || residuals.non_zero_uv != 0;
            let matrix = matrices.get(usize::from(block.segment)).ok_or_else(|| {
                DecodeError::at(
                    DecodeErrorKind::InvalidBitstream,
                    mode_bits.bytes_consumed(),
                    "VP8 macroblock segment exceeds four-entry quantizer table",
                )
            })?;
            let pixels = reconstruct_intra_macroblock(
                block,
                &residuals,
                *matrix,
                image.edges(macroblock_x, macroblock_y),
            )?;
            image.store_macroblock(macroblock_x, macroblock_y, pixels);
            let strength = strengths[usize::from(block.segment)]
                [usize::from(matches!(block.luma, LumaMode::FourByFour(_)))];
            row_filters[macroblock_x] = (
                strength,
                strength.filters_inner(
                    matches!(block.luma, LumaMode::FourByFour(_)),
                    !has_residuals,
                ),
            );
        }
        for (macroblock_x, &(strength, filters_inner)) in row_filters.iter().enumerate() {
            filter_macroblock(MacroblockFilter {
                y: &mut image.y,
                u: &mut image.u,
                v: &mut image.v,
                y_stride: image.y_stride,
                uv_stride: image.uv_stride,
                macroblock_x,
                macroblock_y,
                simple: layout.header.filter.simple,
                strength,
                filters_inner,
            });
        }
    }
    Ok(image)
}

fn macroblock_dimensions(frame: &Vp8Header) -> Result<(usize, usize), DecodeError> {
    let width = usize::try_from(frame.width).map_err(|_| allocation_size_error())?;
    let height = usize::try_from(frame.height).map_err(|_| allocation_size_error())?;
    let macroblock_width = width.checked_add(15).ok_or_else(allocation_size_error)? / 16;
    let macroblock_height = height.checked_add(15).ok_or_else(allocation_size_error)? / 16;
    Ok((macroblock_width, macroblock_height))
}

fn allocation_size_error() -> DecodeError {
    DecodeError::new(
        DecodeErrorKind::LimitExceeded,
        None,
        "VP8 frame allocation size overflows",
    )
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

fn vp8_yuv_to_rgb(y: u8, u: u8, v: u8) -> [u8; 3] {
    let multiply_high = |value: u8, coefficient: i32| (i32::from(value) * coefficient) >> 8;
    let clip = |value: i32| {
        if (0..(256 << 6)).contains(&value) {
            (value >> 6) as u8
        } else if value < 0 {
            0
        } else {
            255
        }
    };
    [
        clip(multiply_high(y, 19_077) + multiply_high(v, 26_149) - 14_234),
        clip(multiply_high(y, 19_077) - multiply_high(u, 6_419) - multiply_high(v, 13_320) + 8_708),
        clip(multiply_high(y, 19_077) + multiply_high(u, 33_050) - 17_685),
    ]
}

#[derive(Clone, Copy)]
struct ChromaSamplingGeometry {
    uv_width: usize,
    uv_height: usize,
    width: usize,
    height: usize,
}

/// Returns one color component after libwebp's fancy 4:2:0 upsampling.
///
/// This mirrors the scalar libwebp line-pair upsampler: first/last picture
/// edges are replicated, while interior 2×2 chroma quads use the same staged
/// integer rounding.
fn fancy_chroma_sample(
    plane: &[u8],
    stride: usize,
    geometry: ChromaSamplingGeometry,
    x: usize,
    y: usize,
) -> u8 {
    let ChromaSamplingGeometry {
        uv_width,
        uv_height,
        width,
        height,
    } = geometry;
    debug_assert!(x < width && y < height);
    debug_assert!(uv_width > 0 && uv_height > 0);
    let (top_row, current_row, top_output) =
        if y == 0 || (height.is_multiple_of(2) && y + 1 == height) {
            let row = y / 2;
            (row, row, true)
        } else if y % 2 == 1 {
            let row = y / 2;
            (row, row + 1, true)
        } else {
            (y / 2 - 1, y / 2, false)
        };
    let sample = |row: usize, column: usize| plane[row * stride + column];

    if x == 0 || (width.is_multiple_of(2) && x + 1 == width) {
        let column = x / 2;
        let top = u16::from(sample(top_row, column));
        let current = u16::from(sample(current_row, column));
        let value = if top_output {
            (3 * top + current + 2) >> 2
        } else {
            (3 * current + top + 2) >> 2
        };
        return value as u8;
    }

    let column = (x - 1) / 2;
    let top_left = u16::from(sample(top_row, column));
    let top = u16::from(sample(top_row, column + 1));
    let left = u16::from(sample(current_row, column));
    let current = u16::from(sample(current_row, column + 1));
    let average = top_left + top + left + current + 8;
    let diagonal_12 = (average + 2 * (top + left)) >> 3;
    let diagonal_03 = (average + 2 * (top_left + current)) >> 3;
    let value = match (top_output, x % 2 == 1) {
        (true, true) => (diagonal_12 + top_left) >> 1,
        (true, false) => (diagonal_03 + top) >> 1,
        (false, true) => (diagonal_03 + left) >> 1,
        (false, false) => (diagonal_12 + current) >> 1,
    };
    value as u8
}
