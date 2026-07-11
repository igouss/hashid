//! The namespace half of an ID.
//!
//! A `Prefix` is a value object: once constructed it is guaranteed to satisfy
//! the grammar, so nothing downstream re-checks it. Two constructors, two
//! different contracts:
//!
//! - [`Prefix::normalize`] is *lenient*, for human/config input: it lowercases,
//!   drops unusable characters, and clamps length. It fails only when nothing
//!   usable survives.
//! - [`Prefix::parse`] is *strict*, for machine input: the text must already be
//!   a legal prefix. `BR` is rejected, not silently folded to `br`.

use core::fmt;

use crate::domain::limits::MAX_PREFIX_LEN;
use crate::domain::slug::Slug;
use crate::error::{IdError, MalformedReason, Result};

/// Characters a prefix may contain, beyond ASCII lowercase alphanumerics.
const PREFIX_PUNCTUATION: [char; 5] = ['_', '-', '.', ':', '#'];

/// A validated ID namespace, e.g. `br`, `my-proj`, or `external:jira`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Prefix(String);

impl Prefix {
    /// Coerce arbitrary human input into a legal prefix.
    ///
    /// Trims, lowercases ASCII, drops every character outside
    /// `[a-z0-9_-.:#]`, clamps to [`MAX_PREFIX_LEN`], and strips trailing
    /// punctuation (which would otherwise produce a doubled separator such as
    /// `br--8cda`).
    ///
    /// # Errors
    ///
    /// [`IdError::UnusablePrefix`] when no usable character survives. The crate
    /// refuses to substitute a default here — see that variant's docs.
    ///
    /// ```
    /// # use hashid::Prefix;
    /// assert_eq!(Prefix::normalize("  Project-Name_2!  ")?.as_str(), "project-name_2");
    /// assert!(Prefix::normalize("!!!").is_err());
    /// # Ok::<(), hashid::IdError>(())
    /// ```
    pub fn normalize(raw: &str) -> Result<Self> {
        let kept: String = raw
            .trim()
            .chars()
            .filter_map(|c: char| {
                let lowered: char = c.to_ascii_lowercase();
                is_prefix_char(lowered).then_some(lowered)
            })
            .take(MAX_PREFIX_LEN)
            .collect();

        let trimmed: &str = kept.trim_end_matches(PREFIX_PUNCTUATION);

        if trimmed.is_empty() {
            return Err(IdError::UnusablePrefix {
                raw: raw.to_owned(),
            });
        }
        Ok(Self(trimmed.to_owned()))
    }

    /// Accept text that is *already* a legal prefix, unchanged.
    ///
    /// # Errors
    ///
    /// [`IdError::Malformed`] carrying the specific [`MalformedReason`].
    pub fn parse(raw: &str) -> Result<Self> {
        if raw.is_empty() {
            return Err(IdError::malformed(raw, MalformedReason::EmptyPrefix));
        }
        if raw.len() > MAX_PREFIX_LEN {
            return Err(IdError::malformed(raw, MalformedReason::PrefixTooLong));
        }
        if !raw.chars().all(is_prefix_char) {
            return Err(IdError::malformed(raw, MalformedReason::PrefixCharset));
        }
        Ok(Self(raw.to_owned()))
    }

    /// Borrow the prefix as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Length in bytes.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Always `false` — a `Prefix` cannot be empty by construction.
    ///
    /// Present only because clippy pairs it with [`Prefix::len`].
    #[must_use]
    pub fn is_empty(&self) -> bool {
        false
    }

    /// Shorten a long prefix to something a human will actually type.
    ///
    /// Prefixes of six characters or fewer are returned unchanged. Longer
    /// multi-word prefixes collapse to their initials (`my_project-name` →
    /// `mpn`); longer single words take their first three alphanumerics
    /// (`superlongname` → `sup`).
    ///
    /// ```
    /// # use hashid::Prefix;
    /// assert_eq!(Prefix::normalize("My_Project-Name")?.abbreviated().as_str(), "mpn");
    /// assert_eq!(Prefix::normalize("superlongname")?.abbreviated().as_str(), "sup");
    /// # Ok::<(), hashid::IdError>(())
    /// ```
    #[must_use]
    pub fn abbreviated(&self) -> Self {
        if self.0.len() <= 6 {
            return self.clone();
        }

        let words: Vec<&str> = self
            .0
            .split(|c: char| !c.is_ascii_alphanumeric())
            .filter(|word: &&str| !word.is_empty())
            .collect();

        // Every word here is non-empty, so each contributes exactly one
        // initial: `initials.len() == words.len()`, which is already > 1.
        if words.len() > 1 {
            return Self(
                words
                    .iter()
                    .filter_map(|word: &&str| word.chars().next())
                    .collect(),
            );
        }

        let head: String = self
            .0
            .chars()
            .filter(char::is_ascii_alphanumeric)
            .take(3)
            .collect();

        if head.is_empty() {
            self.clone()
        } else {
            Self(head)
        }
    }

    /// Append a slug, yielding the composite prefix of a slugged ID.
    ///
    /// `br` + `survey-my-thing` becomes the prefix `br-survey-my-thing`, so
    /// that `br-survey-my-thing-8cda` parses back with the slug intact.
    ///
    /// Returns `None` when the slug cannot fit inside the remaining
    /// [`MAX_PREFIX_LEN`] budget even after truncation — the caller must then
    /// fall back to a hash-only ID rather than emit an oversized prefix.
    #[must_use]
    pub fn joined_with(&self, slug: &Slug) -> Option<Self> {
        // One byte of the budget is spent on the `-` joining prefix to slug.
        let budget: usize = MAX_PREFIX_LEN.checked_sub(self.0.len() + 1)?;
        let fitted: Slug = slug.truncated_to(budget)?;
        Some(Self(format!("{}-{}", self.0, fitted.as_str())))
    }
}

/// Is `c` legal inside a prefix?
fn is_prefix_char(c: char) -> bool {
    c.is_ascii_lowercase() || c.is_ascii_digit() || PREFIX_PUNCTUATION.contains(&c)
}

impl fmt::Display for Prefix {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for Prefix {
    fn as_ref(&self) -> &str {
        &self.0
    }
}
