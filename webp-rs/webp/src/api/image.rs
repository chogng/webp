//! Public static-image and incremental-decoding models.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImageInfo {
    pub width: u32,
    pub height: u32,
    pub has_alpha: bool,
    pub is_animated: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Progress {
    NeedMoreData,
    Complete,
}
