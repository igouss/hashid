//! The crate's error taxonomy.
//!
//! Every failure mode is a distinct, inspectable variant. In particular a
//! malformed ID reports *which* grammar rule it broke via [`MalformedReason`]
//! rather than collapsing to a single stringly "invalid id".

use thiserror::Error;

use crate::domain::limits::{MAX_HASH_LEN, MAX_PREFIX_LEN};
use crate::store::StoreError;

/// Convenience alias for results carrying an [`IdError`].
pub type Result<T> = core::result::Result<T, IdError>;

/// Anything that can go wrong while minting or parsing an ID.
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

    /// A single hash length is outside `1..=MAX_HASH_LEN`.
    ///
    /// Distinct from [`IdError::InvalidHashBounds`], which reports an
    /// unsatisfiable *range*; this one reports a single unrenderable length.
    #[error("invalid hash length {value} (require 1 <= length <= {limit})")]
    InvalidHashLength {
        /// The length the caller supplied.
        value: usize,
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

    /// The instant has no representation as nanoseconds since the Unix epoch.
    ///
    /// Refused rather than coerced. The number is hashed verbatim, so a silent
    /// fallback would hash one value for every out-of-range instant, minting a
    /// single colliding ID for all of them.
    #[error("`{timestamp}` is outside the representable nanosecond range (1677-09-21 ..= 2262-04-11)")]
    TimestampOutOfRange {
        /// The offending instant, as RFC 3339.
        timestamp: String,
    },

    /// Your store could not answer a lookup.
    #[error(transparent)]
    Store(#[from] StoreError),
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
    #[must_use]
    pub fn malformed(id: &str, reason: MalformedReason) -> Self {
        Self::Malformed {
            id: id.to_owned(),
            reason,
        }
    }
}
