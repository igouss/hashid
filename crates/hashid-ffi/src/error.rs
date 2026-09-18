//! The error taxonomy, restated as a foreign-language-friendly enum.
//!
//! This is deliberately a *separate* type from `hashid::IdError`, not a
//! re-export. The core error is `#[non_exhaustive]` and free to grow; the FFI
//! error is a published wire contract that foreign code writes `match`
//! statements against. Translating between them in one place (`From`, below)
//! is what lets the core evolve without silently changing what a Kotlin
//! `when` block sees.

use hashid::{IdError, MalformedReason as CoreReason, StoreError as CoreStoreError};

use crate::num::{report, widen};

/// Which rule of the ID grammar an input broke.
///
/// Mirrors `hashid::MalformedReason`. The `Unspecified` variant exists so
/// that a core release which adds a grammar rule cannot break already-compiled
/// foreign code: an unknown reason degrades to `Unspecified` rather than
/// widening this enum under the caller's feet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum MalformedReason {
    /// The input was empty, or nothing but whitespace.
    Empty,
    /// No `-` separating prefix from hash.
    MissingSeparator,
    /// Nothing before the final `-`.
    EmptyPrefix,
    /// Prefix longer than `crate::limits``().max_prefix_len`.
    PrefixTooLong,
    /// Prefix contains a character outside `[a-z0-9_-.:#]`.
    PrefixCharset,
    /// Nothing after the final `-`.
    EmptyHash,
    /// Hash longer than `crate::limits``().max_hash_len`.
    HashTooLong,
    /// Hash contains a character outside base36 `[a-z0-9]`.
    HashCharset,
    /// A `.`-separated child segment is not a base-10 integer.
    ChildSegment,
    /// A grammar rule newer than this binding. Read `HashidError.message`.
    Unspecified,
}

impl core::fmt::Display for MalformedReason {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let text: &str = match self {
            Self::Empty => "input is empty",
            Self::MissingSeparator => "missing `-` separator between prefix and hash",
            Self::EmptyPrefix => "empty prefix",
            Self::PrefixTooLong => "prefix is too long",
            Self::PrefixCharset => "prefix contains characters outside [a-z0-9_-.:#]",
            Self::EmptyHash => "empty hash",
            Self::HashTooLong => "hash is too long",
            Self::HashCharset => "hash contains characters outside base36 [a-z0-9]",
            Self::ChildSegment => "child path segment is not a base-10 integer",
            Self::Unspecified => "violates a grammar rule this binding does not know",
        };
        f.write_str(text)
    }
}

impl From<CoreReason> for MalformedReason {
    fn from(reason: CoreReason) -> Self {
        match reason {
            CoreReason::Empty => Self::Empty,
            CoreReason::MissingSeparator => Self::MissingSeparator,
            CoreReason::EmptyPrefix => Self::EmptyPrefix,
            CoreReason::PrefixTooLong => Self::PrefixTooLong,
            CoreReason::PrefixCharset => Self::PrefixCharset,
            CoreReason::EmptyHash => Self::EmptyHash,
            CoreReason::HashTooLong => Self::HashTooLong,
            CoreReason::HashCharset => Self::HashCharset,
            CoreReason::ChildSegment => Self::ChildSegment,
            _ => Self::Unspecified,
        }
    }
}

/// Anything that can go wrong while minting or parsing an ID.
///
/// Every variant carries the data a caller needs to *act*, not merely to log:
/// `Malformed` names the rule that broke, so a form can highlight one field
/// rather than reject the whole input.
#[derive(Debug, thiserror::Error, uniffi::Error)]
#[non_exhaustive]
pub enum HashidError {
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
    #[error("prefix `{raw}` contains no characters usable in an id")]
    UnusablePrefix {
        /// The raw prefix the caller supplied.
        raw: String,
    },

    /// Hash-length bounds are not a satisfiable range.
    #[error("invalid hash length bounds min={min}, max={max} (require 1 <= min <= max <= {limit})")]
    InvalidHashBounds {
        /// Requested lower bound.
        min: i64,
        /// Requested upper bound.
        max: i64,
        /// The hard ceiling the grammar imposes.
        limit: i64,
    },

    /// A single hash length is outside `1..=limits().max_hash_len`.
    ///
    /// Distinct from `HashidError::InvalidHashBounds`, which reports an
    /// unsatisfiable *range*; this one reports a single unrenderable length.
    #[error("invalid hash length {value} (require 1 <= length <= {limit})")]
    InvalidHashLength {
        /// The length the caller supplied.
        value: i64,
        /// The hard ceiling the grammar imposes.
        limit: i64,
    },

    /// The target collision probability is not a probability.
    #[error("invalid collision probability {value} (require 0.0 < p < 1.0)")]
    InvalidCollisionProb {
        /// The value the caller supplied.
        value: f64,
    },

    /// The generator exhausted its escalation ladder without finding a free ID.
    ///
    /// Returned *instead of* an ID already known to be taken.
    #[error("id space exhausted for prefix `{prefix}` after {attempts} attempts")]
    Exhausted {
        /// The prefix that ran out of room.
        prefix: String,
        /// How many distinct candidates were rejected.
        attempts: i64,
    },

    /// A number crossing the boundary is outside the range its field allows.
    ///
    /// Reported rather than clamped. A saturating conversion turns a mistake
    /// into a plausible number — a negative population becomes zero, a nonce
    /// of `-1` becomes four billion — and tells the caller nothing.
    #[error("`{field}` is out of range: {value}")]
    OutOfRange {
        /// Which field carried it.
        field: String,
        /// The value that arrived.
        value: i64,
    },

    /// The instant has no representation as nanoseconds since the Unix epoch.
    #[error("`{timestamp}` is outside the representable nanosecond range")]
    TimestampOutOfRange {
        /// The offending instant, as RFC 3339.
        timestamp: String,
    },

    /// A failure this version of the binding has no name for.
    ///
    /// The core's error type is `#[non_exhaustive]`, so it may grow a variant
    /// this binding predates. Reported as itself rather than folded into a
    /// neighbouring variant, which would send a caller looking in the wrong
    /// place; the message still carries the real description.
    #[error("{message}")]
    Unknown {
        /// What the core reported.
        message: String,
    },

    /// The caller's own store failed to answer a lookup.
    ///
    /// Reported instead of a guess. An unreachable database is not the same
    /// answer as a free ID, and the minter propagates rather than inventing an
    /// ID that may already exist.
    #[error("id store lookup failed: {message}")]
    Store {
        /// What the store reported.
        message: String,
    },
}

impl From<IdError> for HashidError {
    fn from(err: IdError) -> Self {
        match err {
            IdError::Malformed { id, reason } => Self::Malformed {
                id,
                reason: reason.into(),
            },
            IdError::PrefixMismatch { expected, found } => Self::PrefixMismatch { expected, found },
            IdError::UnusablePrefix { raw } => Self::UnusablePrefix { raw },
            IdError::InvalidHashBounds { min, max, limit } => Self::InvalidHashBounds {
                min: report(min),
                max: report(max),
                limit: report(limit),
            },
            IdError::InvalidHashLength { value, limit } => Self::InvalidHashLength {
                value: report(value),
                limit: report(limit),
            },
            IdError::InvalidCollisionProb { value } => Self::InvalidCollisionProb { value },
            IdError::Exhausted { prefix, attempts } => Self::Exhausted {
                prefix,
                attempts: widen(attempts),
            },
            IdError::TimestampOutOfRange { timestamp } => Self::TimestampOutOfRange { timestamp },
            IdError::Store(inner) => Self::Store {
                message: inner.message().to_owned(),
            },
            // `IdError` is `#[non_exhaustive]`: a variant added upstream lands
            // here rather than breaking the build. Reported as what it is —
            // unrecognised — rather than folded into `Store`, which would send
            // a caller to look at their database over someone else's bug.
            other => Self::Unknown {
                message: other.to_string(),
            },
        }
    }
}

/// A lookup against the caller's own ID store failed.
///
/// Foreign implementations of `IdStore` raise this to say "I do not know" —
/// which the
/// generator propagates rather than guessing. Answering `false` would invent a
/// free ID that may already be taken; answering `true` would burn a good
/// candidate. Both are silent, so neither is offered.
#[derive(Debug, thiserror::Error, uniffi::Error)]
#[non_exhaustive]
pub enum StoreError {
    /// The store could not answer.
    #[error("{message}")]
    Failed {
        /// What went wrong, in the store's own words.
        message: String,
    },
}

impl StoreError {
    /// A failure described by `message`.
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self::Failed {
            message: message.into(),
        }
    }
}

impl From<uniffi::UnexpectedUniFFICallbackError> for StoreError {
    fn from(err: uniffi::UnexpectedUniFFICallbackError) -> Self {
        Self::new(err.reason)
    }
}

impl From<StoreError> for CoreStoreError {
    fn from(err: StoreError) -> Self {
        Self::new(err.to_string())
    }
}
