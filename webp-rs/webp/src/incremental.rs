//! Bounded incremental-input buffering.

use crate::DecodeOptions;
use crate::Image;
use crate::Progress;
use crate::decode;
use webp_core::DecodeError;
use webp_core::DecodeErrorKind;

#[derive(Debug, Clone)]
pub struct IncrementalDecoder {
    options: DecodeOptions,
    bytes: Vec<u8>,
    terminal: bool,
}

impl IncrementalDecoder {
    #[must_use]
    pub fn new(options: DecodeOptions) -> Self {
        Self {
            options,
            bytes: Vec::new(),
            terminal: false,
        }
    }

    pub fn push(&mut self, bytes: &[u8]) -> Result<Progress, DecodeError> {
        if self.terminal {
            return Err(DecodeError::at(
                DecodeErrorKind::InvalidParameter,
                self.bytes.len(),
                "push after finish",
            ));
        }
        let total = self.bytes.len().checked_add(bytes.len()).ok_or_else(|| {
            DecodeError::at(
                DecodeErrorKind::LimitExceeded,
                self.bytes.len(),
                "incremental input size overflow",
            )
        })?;
        if total > self.options.limits.max_input_bytes {
            return Err(DecodeError::at(
                DecodeErrorKind::LimitExceeded,
                total,
                "incremental input exceeds max_input_bytes",
            ));
        }
        self.bytes.try_reserve(bytes.len()).map_err(|_| {
            DecodeError::at(
                DecodeErrorKind::AllocationFailed,
                self.bytes.len(),
                "cannot reserve incremental input",
            )
        })?;
        self.bytes.extend_from_slice(bytes);
        Ok(Progress::NeedMoreData)
    }

    pub fn finish(mut self) -> Result<Image, DecodeError> {
        self.terminal = true;
        decode(&self.bytes, &self.options)
    }
}
