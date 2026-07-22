//! Container-specific parse and serialization errors.

/// Stable category for a WebP container failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContainerErrorKind {
    /// The input ends before a declared RIFF or chunk field is complete.
    UnexpectedEof,
    /// RIFF framing, chunk layout, flags, or animation geometry is invalid.
    InvalidContainer,
    /// A configured container resource limit was exceeded.
    LimitExceeded,
    /// A RIFF or WebP container size cannot be represented.
    SizeOverflow,
    /// Output storage could not be reserved.
    AllocationFailed,
    /// A canvas dimension required by an extended container is invalid.
    InvalidDimensions,
    /// Animation geometry or wire fields are invalid.
    InvalidAnimation,
}

/// Error owned by the WebP RIFF container boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContainerError {
    kind: ContainerErrorKind,
    offset: Option<usize>,
    context: &'static str,
}

impl ContainerError {
    #[doc(hidden)]
    pub const fn new(kind: ContainerErrorKind, context: &'static str) -> Self {
        Self {
            kind,
            offset: None,
            context,
        }
    }

    #[doc(hidden)]
    pub const fn at(kind: ContainerErrorKind, offset: usize, context: &'static str) -> Self {
        Self {
            kind,
            offset: Some(offset),
            context,
        }
    }

    /// Returns the stable error category.
    #[must_use]
    pub const fn kind(self) -> ContainerErrorKind {
        self.kind
    }

    /// Returns the operation that failed.
    #[must_use]
    pub const fn context(self) -> &'static str {
        self.context
    }

    /// Returns the byte offset associated with a parse failure, when known.
    #[must_use]
    pub const fn offset(self) -> Option<usize> {
        self.offset
    }
}

impl core::fmt::Display for ContainerError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "{:?}: {}", self.kind, self.context)?;
        if let Some(offset) = self.offset {
            write!(formatter, " at byte offset {offset}")?;
        }
        Ok(())
    }
}

impl std::error::Error for ContainerError {}
