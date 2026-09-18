//! The uniquifying half of an ID.

use core::fmt;

use crate::domain::limits::MAX_HASH_LEN;
use crate::error::{IdError, MalformedReason, Result};

/// A validated base36 (`[0-9a-z]`) hash, non-empty and within
/// [`MAX_HASH_LEN`].
///
/// Producing one from content is the job of [`digest()`](crate::generate::digest()); this
/// type only guarantees the shape.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Base36Hash(String);

impl Base36Hash {
    /// Accept text that is already a legal base36 hash.
    ///
    /// # Errors
    ///
    /// [`IdError::Malformed`] carrying the specific [`MalformedReason`].
    pub fn parse(raw: &str) -> Result<Self> {
        if raw.is_empty() {
            return Err(IdError::malformed(raw, MalformedReason::EmptyHash));
        }
        if raw.len() > MAX_HASH_LEN {
            return Err(IdError::malformed(raw, MalformedReason::HashTooLong));
        }
        if !raw
            .chars()
            .all(|c: char| c.is_ascii_lowercase() || c.is_ascii_digit())
        {
            return Err(IdError::malformed(raw, MalformedReason::HashCharset));
        }
        Ok(Self(raw.to_owned()))
    }

    /// Borrow the hash as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Length in bytes.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Always `false` — a `Base36Hash` cannot be empty by construction.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        false
    }
}

impl fmt::Display for Base36Hash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for Base36Hash {
    fn as_ref(&self) -> &str {
        &self.0
    }
}
