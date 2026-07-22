//! WebP chunk identifiers.

/// A four-byte chunk identifier.
pub type FourCc = [u8; 4];

pub const VP8: FourCc = *b"VP8 ";
pub const VP8L: FourCc = *b"VP8L";
pub const VP8X: FourCc = *b"VP8X";
pub const ALPH: FourCc = *b"ALPH";
pub const ICCP: FourCc = *b"ICCP";
pub const EXIF: FourCc = *b"EXIF";
pub const XMP: FourCc = *b"XMP ";
pub const ANIM: FourCc = *b"ANIM";
pub const ANMF: FourCc = *b"ANMF";

pub(crate) fn is_known(fourcc: FourCc) -> bool {
    matches!(
        fourcc,
        VP8 | VP8L | VP8X | ALPH | ICCP | EXIF | XMP | ANIM | ANMF
    )
}
