//! The failure a registry adapter is allowed to report.

use core::fmt;
use std::error::Error;

/// A lookup against the caller's ID store failed.
///
/// The domain does not care *why* — a dropped socket, a corrupt page, a
/// permissions error are all the same to it. It only cares that the answer to
/// "does this ID exist?" is genuinely unknown, and that it must therefore not
/// pretend otherwise.
///
/// This type is the reason the [`IdRegistry`](crate::port::IdRegistry) port is
/// fallible. An infallible `Fn(&Id) -> bool` port forces every adapter with a
/// real backing store into the same lie: swallow the error and answer `true`
/// or `false`. Answering `false` invents a collision; answering `true` burns a
/// candidate. Both are silent.
#[derive(Debug)]
pub struct RegistryError {
    message: String,
    source: Option<Box<dyn Error + Send + Sync + 'static>>,
}

impl RegistryError {
    /// A lookup failure described only by a message.
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            source: None,
        }
    }

    /// A lookup failure wrapping the underlying cause, preserving the chain.
    #[must_use]
    pub fn with_source(
        message: impl Into<String>,
        source: impl Error + Send + Sync + 'static,
    ) -> Self {
        Self {
            message: message.into(),
            source: Some(Box::new(source)),
        }
    }

    /// The human-readable description of the failure.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for RegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "id registry lookup failed: {}", self.message)
    }
}

impl Error for RegistryError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.source
            .as_ref()
            .map(|boxed| -> &(dyn Error + 'static) { boxed.as_ref() })
    }
}
