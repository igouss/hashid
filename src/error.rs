//! The crate's error taxonomy.
//!
//! Every failure mode is a distinct, inspectable variant. In particular a
//! malformed ID reports *which* grammar rule it broke via [`MalformedReason`]
//! rather than collapsing to a single stringly "invalid id".

use thiserror::Error;

use crate::domain::limits::{MAX_HASH_LEN, MAX_PREFIX_LEN};
use crate::port::RegistryError;

/// Convenience alias for results carrying an [`IdError`].
pub type Result<T> = core::result::Result<T, IdError>;

/// Anything that can go wrong while minting, parsing, or resolving an ID.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum IdError {
    /// The input does not satisfy the ID grammar.
    #[error("malformed id `{id}`: {reason}")]
    Malformed {
        /// The offending input, verbatim.
        id: String,
        /// The specific grammar rule that was broken.
        reason: MalformedReason,
    },

    /// The ID parsed, but carries a prefix the caller does not accept.
    #[error("prefix mismatch: expected `{expected}`, found `{found}`")]
    PrefixMismatch {
        /// The prefix the caller required.
        expected: String,
        /// The prefix the ID actually carries.
        found: String,
    },

    /// A partial ID matched more than one known ID.
    #[error("`{fragment}` is ambiguous: {} ids match ({})", .matches.len(), .matches.join(", "))]
    Ambiguous {
        /// The fragment the caller supplied.
        fragment: String,
        /// Every ID the fragment matched, so the caller can present a choice.
        matches: Vec<String>,
    },

    /// A partial ID matched nothing.
    #[error("no id matches `{fragment}`")]
    NotFound {
        /// The fragment the caller supplied.
        fragment: String,
    },

    /// A prefix normalized away to nothing — no usable characters survived.
    ///
    /// The crate deliberately refuses to invent a substitute prefix here: a
    /// silent fallback would attach real IDs to a name the caller never chose.
    #[error("prefix `{raw}` contains no characters usable in an id")]
    UnusablePrefix {
        /// The raw prefix the caller supplied.
        raw: String,
    },

    /// Hash-length bounds are not a satisfiable range.
    #[error(
        "invalid hash length bounds min={min}, max={max} (require 1 <= min <= max <= {limit})"
    )]
    InvalidHashBounds {
        /// Requested lower bound.
        min: usize,
        /// Requested upper bound.
        max: usize,
        /// The hard ceiling the grammar imposes ([`MAX_HASH_LEN`]).
        limit: usize,
    },

    /// The target collision probability is not a probability.
    #[error("invalid collision probability {value} (require 0.0 < p < 1.0)")]
    InvalidCollisionProb {
        /// The value the caller supplied.
        value: f64,
    },

    /// The generator exhausted its escalation ladder without finding a free ID.
    ///
    /// This is the honest end of the road: it is returned *instead of* handing
    /// back an ID already known to be taken.
    #[error("id space exhausted for prefix `{prefix}` after {attempts} attempts")]
    Exhausted {
        /// The prefix that ran out of room.
        prefix: String,
        /// How many distinct candidates were rejected.
        attempts: u32,
    },

    /// The caller's registry adapter failed to answer a lookup.
    #[error(transparent)]
    Registry(#[from] RegistryError),
}

/// Precisely which rule of the ID grammar an input violated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum MalformedReason {
    /// The input was empty, or nothing but whitespace.
    #[error("input is empty")]
    Empty,

    /// No `-` separating prefix from hash.
    #[error("missing `-` separator between prefix and hash")]
    MissingSeparator,

    /// Nothing before the final `-`.
    #[error("empty prefix")]
    EmptyPrefix,

    /// Prefix longer than [`MAX_PREFIX_LEN`].
    #[error("prefix exceeds {MAX_PREFIX_LEN} characters")]
    PrefixTooLong,

    /// Prefix contains a character outside `[a-z0-9_-.:#]`.
    #[error("prefix contains characters outside [a-z0-9_-.:#]")]
    PrefixCharset,

    /// Nothing after the final `-`.
    #[error("empty hash")]
    EmptyHash,

    /// Hash longer than [`MAX_HASH_LEN`].
    #[error("hash exceeds {MAX_HASH_LEN} characters")]
    HashTooLong,

    /// Hash contains a character outside base36 `[a-z0-9]`.
    #[error("hash contains characters outside base36 [a-z0-9]")]
    HashCharset,

    /// A `.`-separated child segment is not a base-10 integer.
    #[error("child path segment is not a base-10 integer")]
    ChildSegment,
}

impl IdError {
    /// Build a [`IdError::Malformed`] for `id` with the given `reason`.
    pub(crate) fn malformed(id: &str, reason: MalformedReason) -> Self {
        Self::Malformed {
            id: id.to_owned(),
            reason,
        }
    }
}
