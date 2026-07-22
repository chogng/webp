//! Container-specific parse and serialization errors.

/// Stable category for a WebP container failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContainerErrorKind {
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
    context: &'static str,
}

impl ContainerError {
    pub(crate) const fn new(kind: ContainerErrorKind, context: &'static str) -> Self {
        Self { kind, context }
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
}

impl core::fmt::Display for ContainerError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(self.context)
    }
}

impl std::error::Error for ContainerError {}
