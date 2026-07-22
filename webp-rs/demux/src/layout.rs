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
    if anmf_count > 0 && (lossy_count > 0 || lossless_count > 0 || alph_count > 0) {
        return Err(error("animated and still-image chunks cannot be mixed"));
    }
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
            || flags.animation() != (anim_count == 1 && anmf_count != 0)
            || (alph_count == 1 && !flags.alpha())
        {
            return Err(ContainerError::at(
                ContainerErrorKind::InvalidContainer,
                first.offset,
                "VP8X flags do not match present chunks",
            ));
        }
    } else if iccp_count != 0
        || exif_count != 0
        || xmp_count != 0
        || alph_count != 0
        || anim_count != 0
        || anmf_count != 0
    {
        return Err(error("extended chunks require VP8X"));
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
