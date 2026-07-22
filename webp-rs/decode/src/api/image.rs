//! Public static-image and incremental-decoding models.

#[cfg(feature = "decode")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

#[cfg(feature = "decode")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImageInfo {
    pub width: u32,
    pub height: u32,
    pub has_alpha: bool,
    pub is_animated: bool,
}

#[cfg(feature = "decode")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Progress {
    /// The container or codec accepted the bytes but no additional output row
    /// is stable yet.
    NeedMoreData,
    /// A larger prefix of the output is now safe to consume.
    DecodedRows { decoded_rows: u32 },
    /// The full static image is decoded and can be taken with `finish`.
    Complete,
}

/// Borrowed view of the stable RGBA prefix produced by an incremental decode.
///
/// `rgba` contains exactly `decoded_rows * width * 4` bytes. Rows outside this
/// prefix can still be changed by VP8 in-loop filtering or chroma upsampling
/// and are intentionally not exposed.
#[cfg(feature = "decode")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IncrementalImage<'a> {
    pub width: u32,
    pub height: u32,
    pub decoded_rows: u32,
    pub rgba: &'a [u8],
}
