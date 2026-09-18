//! The policy that governs how forgiving resolution is.

use hashid::Prefix;

/// How to turn what a human typed into the ID they meant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolverConfig {
    default_prefix: Prefix,
    allow_substring_match: bool,
}

impl ResolverConfig {
    /// Resolve against `default_prefix`, with substring matching enabled.
    #[must_use]
    pub fn new(default_prefix: Prefix) -> Self {
        Self {
            default_prefix,
            allow_substring_match: true,
        }
    }

    /// Turn substring matching on or off.
    ///
    /// With it off, resolution accepts only a complete ID or a complete hash —
    /// no guessing from a fragment. Worth doing in a non-interactive context
    /// where an ambiguous abbreviation is a footgun rather than a convenience.
    #[must_use]
    pub fn with_substring_match(mut self, allow: bool) -> Self {
        self.allow_substring_match = allow;
        self
    }

    /// The prefix prepended to a bare hash.
    #[must_use]
    pub fn default_prefix(&self) -> &Prefix {
        &self.default_prefix
    }

    /// Is substring matching enabled?
    #[must_use]
    pub fn allow_substring_match(&self) -> bool {
        self.allow_substring_match
    }
}
