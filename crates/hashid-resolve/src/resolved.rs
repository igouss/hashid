//! The outcome of turning a partial, human-typed fragment into a real ID.

use hashid::Id;

/// How a fragment was matched to an ID.
///
/// Worth surfacing to users: a `Substring` match on `ab` is a very different
/// claim from an `Exact` match on `br-abc123`, and a CLI may reasonably want to
/// echo back what it inferred.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MatchType {
    /// The input was already a complete, existing ID.
    Exact,
    /// The input was a bare hash; the default prefix was prepended.
    PrefixNormalized,
    /// The input was a fragment of the hash, matching exactly one ID.
    Substring,
}

/// A fragment, resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedId {
    /// The full ID the fragment resolved to.
    pub id: Id,
    /// How it was matched.
    pub match_type: MatchType,
    /// The caller's original input, verbatim — for echoing back in messages.
    pub input: String,
}
