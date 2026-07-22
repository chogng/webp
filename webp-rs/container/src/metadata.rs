//! Borrowed WebP metadata.

/// Raw metadata selected from the first chunk of each type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Metadata<'a> {
    pub iccp: Option<&'a [u8]>,
    pub exif: Option<&'a [u8]>,
    pub xmp: Option<&'a [u8]>,
}
