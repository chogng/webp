//! Borrowed WebP metadata.

/// Raw metadata selected from the first chunk of each type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Metadata<'a> {
    pub iccp: Option<&'a [u8]>,
    pub exif: Option<&'a [u8]>,
    pub xmp: Option<&'a [u8]>,
}

/// Owned raw WebP metadata payloads.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OwnedMetadata {
    pub iccp: Option<Vec<u8>>,
    pub exif: Option<Vec<u8>>,
    pub xmp: Option<Vec<u8>>,
}

impl OwnedMetadata {
    /// Borrows the owned payloads for container parsing or serialization.
    #[must_use]
    pub fn borrowed(&self) -> Metadata<'_> {
        Metadata {
            iccp: self.iccp.as_deref(),
            exif: self.exif.as_deref(),
            xmp: self.xmp.as_deref(),
        }
    }
}
