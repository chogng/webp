//! Strict top-level WebP chunk-layout validation.

use crate::ALPH;
use crate::ANIM;
use crate::ANMF;
use crate::Chunk;
use crate::ContainerError;
use crate::ContainerErrorKind;
use crate::EXIF;
use crate::ICCP;
use crate::VP8;
use crate::VP8L;
use crate::VP8X;
use crate::Vp8x;
use crate::XMP;

const RIFF_HEADER_LEN: usize = 12;

pub(crate) fn validate_strict_layout(
    chunks: &[Chunk<'_>],
    vp8x: Option<Vp8x>,
) -> Result<(), ContainerError> {
    let mut lossy_count = 0_u32;
    let mut lossless_count = 0_u32;
    let mut alph_count = 0_u32;
    let mut iccp_count = 0_u32;
    let mut exif_count = 0_u32;
    let mut xmp_count = 0_u32;
    let mut anim_count = 0_u32;
    let mut anmf_count = 0_u32;
    for chunk in chunks {
        match chunk.fourcc {
            VP8 => lossy_count += 1,
            VP8L => lossless_count += 1,
            ALPH => alph_count += 1,
            ICCP => iccp_count += 1,
            EXIF => exif_count += 1,
            XMP => xmp_count += 1,
            ANIM => anim_count += 1,
            ANMF => anmf_count += 1,
            _ => {}
        }
    }
    if lossy_count > 1
        || lossless_count > 1
        || alph_count > 1
        || iccp_count > 1
        || exif_count > 1
        || xmp_count > 1
        || anim_count > 1
    {
        return Err(error("duplicate singleton chunk"));
    }
    if lossy_count > 0 && lossless_count > 0 {
        return Err(error("both VP8 and VP8L chunks present"));
    }
    let image_count = lossy_count + lossless_count;
    if let Some(header) = vp8x {
        let first = chunks.first().expect("VP8X has a source chunk");
        if first.fourcc != VP8X {
            return Err(ContainerError::at(
                ContainerErrorKind::InvalidContainer,
                first.offset,
                "VP8X must be the first chunk",
            ));
        }
        let flags = header.flags;
        if flags.iccp() != (iccp_count == 1)
            || flags.exif() != (exif_count == 1)
            || flags.xmp() != (xmp_count == 1)
            || (alph_count == 1 && !flags.alpha())
        {
            return Err(ContainerError::at(
                ContainerErrorKind::InvalidContainer,
                first.offset,
                "VP8X flags do not match present chunks",
            ));
        }
        if flags.animation() {
            if anim_count != 1 || anmf_count == 0 {
                return Err(error("animated WebP requires ANIM and ANMF chunks"));
            }
            if image_count != 0 || alph_count != 0 {
                return Err(error("animated and still-image chunks cannot be mixed"));
            }
            validate_animation_order(chunks)?;
        } else {
            if anim_count != 0 || anmf_count != 0 {
                return Err(error("animation chunks require the VP8X animation flag"));
            }
            validate_static_layout(chunks, image_count, lossy_count, lossless_count, alph_count)?;
            if lossy_count == 1 && flags.alpha() != (alph_count == 1) {
                return Err(error("VP8X alpha flag does not match the VP8/ALPH layout"));
            }
        }
    } else if iccp_count != 0
        || exif_count != 0
        || xmp_count != 0
        || alph_count != 0
        || anim_count != 0
        || anmf_count != 0
    {
        return Err(error("extended chunks require VP8X"));
    } else {
        validate_static_layout(chunks, image_count, lossy_count, lossless_count, alph_count)?;
    }
    Ok(())
}

fn validate_static_layout(
    chunks: &[Chunk<'_>],
    image_count: u32,
    lossy_count: u32,
    lossless_count: u32,
    alph_count: u32,
) -> Result<(), ContainerError> {
    if image_count != 1 {
        return Err(error("static WebP requires exactly one VP8 or VP8L chunk"));
    }
    if alph_count != 0 && (lossy_count != 1 || lossless_count != 0) {
        return Err(error("ALPH requires a VP8 lossy bitstream"));
    }
    let image_index = chunks
        .iter()
        .position(|chunk| matches!(chunk.fourcc, VP8 | VP8L))
        .expect("image_count is one");
    if let Some(alpha_index) = chunks.iter().position(|chunk| chunk.fourcc == ALPH)
        && alpha_index > image_index
    {
        return Err(error("ALPH must appear before the VP8 bitstream"));
    }
    validate_iccp_before_image(chunks, image_index)
}

fn validate_animation_order(chunks: &[Chunk<'_>]) -> Result<(), ContainerError> {
    let anim_index = chunks
        .iter()
        .position(|chunk| chunk.fourcc == ANIM)
        .expect("validated ANIM count");
    if chunks
        .iter()
        .position(|chunk| chunk.fourcc == ANMF)
        .is_some_and(|first_frame| first_frame < anim_index)
    {
        return Err(error("ANIM must appear before every ANMF frame"));
    }
    validate_iccp_before_image(chunks, anim_index)
}

fn validate_iccp_before_image(
    chunks: &[Chunk<'_>],
    image_start: usize,
) -> Result<(), ContainerError> {
    if chunks
        .iter()
        .position(|chunk| chunk.fourcc == ICCP)
        .is_some_and(|iccp| iccp > image_start)
    {
        return Err(error("ICCP must appear before image data"));
    }
    Ok(())
}

fn error(context: &'static str) -> ContainerError {
    ContainerError::at(
        ContainerErrorKind::InvalidContainer,
        RIFF_HEADER_LEN,
        context,
    )
}
