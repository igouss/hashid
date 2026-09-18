//! The optional human-readable segment of an ID.
//!
//! A slug turns `br-8cda` into `br-survey-my-thing-8cda`: still unique (the
//! hash is always appended), but recognisable in a commit message.

use core::fmt;

use crate::domain::limits::MAX_SLUG_LEN;

/// A normalized, non-empty, ID-safe label.
///
/// Construct via [`Slug::normalize`]. There is no lossless constructor: a slug
/// is a *rendering* of arbitrary human text, never a value the caller asserts.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Slug(String);

impl Slug {
    /// Render arbitrary text as a slug.
    ///
    /// Lowercases ASCII, keeps `[a-z0-9]`, collapses every other run to a
    /// single `-`, strips leading and trailing `-`, and caps the result at
    /// [`MAX_SLUG_LEN`].
    ///
    /// Non-ASCII characters are *dropped*, not transliterated: `café-résumé`
    /// becomes `caf-r-sum`. Transliteration would need a Unicode table the
    /// domain has no business carrying.
    ///
    /// Returns `None` when nothing usable survives — the caller must then fall
    /// back to a hash-only ID.
    ///
    /// ```
    /// # use hashid::Slug;
    /// assert_eq!(Slug::normalize("Hello World!").unwrap().as_str(), "hello-world");
    /// assert_eq!(Slug::normalize("a   b/c.d!!e").unwrap().as_str(), "a-b-c-d-e");
    /// assert!(Slug::normalize("!!!").is_none());
    /// ```
    #[must_use]
    pub fn normalize(raw: &str) -> Option<Self> {
        let mut out: String = String::with_capacity(raw.len());
        let mut pending_separator: bool = false;

        for c in raw.chars() {
            let lowered: char = c.to_ascii_lowercase();
            if lowered.is_ascii_lowercase() || lowered.is_ascii_digit() {
                out.push(lowered);
                pending_separator = false;
            } else if !pending_separator && !out.is_empty() {
                out.push('-');
                pending_separator = true;
            }
        }

        trim_trailing_hyphens(&mut out);
        if out.len() > MAX_SLUG_LEN {
            out.truncate(MAX_SLUG_LEN);
            trim_trailing_hyphens(&mut out);
        }

        (!out.is_empty()).then_some(Self(out))
    }

    /// Shorten this slug to at most `max` bytes, re-trimming any hyphen the cut
    /// exposes.
    ///
    /// Returns `None` when `max` leaves no room for a non-empty slug.
    #[must_use]
    pub fn truncated_to(&self, max: usize) -> Option<Self> {
        if self.0.len() <= max {
            return Some(self.clone());
        }
        let mut out: String = self.0.clone();
        out.truncate(max);
        trim_trailing_hyphens(&mut out);
        (!out.is_empty()).then_some(Self(out))
    }

    /// Borrow the slug as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Length in bytes.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Always `false` — a `Slug` cannot be empty by construction.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        false
    }
}

/// Drop every trailing `-` from `s`, in place.
fn trim_trailing_hyphens(s: &mut String) {
    while s.ends_with('-') {
        s.pop();
    }
}

impl fmt::Display for Slug {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for Slug {
    fn as_ref(&self) -> &str {
        &self.0
    }
}
