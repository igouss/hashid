//! What resolution can fail with.

use thiserror::Error;

use hashid::{IdError, StoreError};

/// Convenience alias for results carrying a [`ResolveError`].
pub type Result<T> = core::result::Result<T, ResolveError>;

/// Anything that can go wrong while resolving a fragment.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ResolveError {
    /// A partial ID matched more than one known ID.
    #[error("`{fragment}` is ambiguous: {} ids match ({})", .matches.len(), .matches.join(", "))]
    Ambiguous {
        /// The fragment the caller supplied.
        fragment: String,
        /// Every ID the fragment matched, so the caller can present a choice.
        matches: Vec<String>,
    },

    /// A partial ID matched nothing.
    ///
    /// Distinct from [`ResolveError::Store`] on purpose: a caller that deletes
    /// on "not found" would very much like to know the difference between an
    /// absent ID and an unreachable database.
    #[error("no id matches `{fragment}`")]
    NotFound {
        /// The fragment the caller supplied.
        fragment: String,
    },

    /// The input was not something that could be resolved at all.
    #[error(transparent)]
    Malformed(#[from] IdError),

    /// Your lookup could not answer.
    #[error(transparent)]
    Store(#[from] StoreError),
}
