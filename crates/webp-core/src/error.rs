use core::fmt;

/// Stable, high-level reason a WebP operation failed.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DecodeErrorKind {
    InvalidContainer,
    InvalidBitstream,
    UnsupportedFeature,
    UnexpectedEof,
    LimitExceeded,
    AllocationFailed,
    InvalidParameter,
}

/// Error returned by the core parsing and decoding primitives.
///
/// `offset` is diagnostic only: callers should branch on [`Self::kind`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodeError {
    kind: DecodeErrorKind,
    offset: Option<usize>,
    context: &'static str,
}

impl DecodeError {
    #[must_use]
    pub const fn new(kind: DecodeErrorKind, offset: Option<usize>, context: &'static str) -> Self {
        Self {
            kind,
            offset,
            context,
        }
    }

    #[must_use]
    pub const fn at(kind: DecodeErrorKind, offset: usize, context: &'static str) -> Self {
        Self::new(kind, Some(offset), context)
    }

    #[must_use]
    pub const fn kind(&self) -> DecodeErrorKind {
        self.kind
    }

    #[must_use]
    pub const fn offset(&self) -> Option<usize> {
        self.offset
    }

    #[must_use]
    pub const fn context(&self) -> &'static str {
        self.context
    }
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.context)?;
        if let Some(offset) = self.offset {
            write!(f, " at byte offset {offset}")?;
        }
        Ok(())
    }
}

impl std::error::Error for DecodeError {}
