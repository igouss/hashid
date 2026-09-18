//! How many base36 characters a hash gets.

use core::fmt;

use crate::domain::limits::MAX_HASH_LEN;
use crate::error::{IdError, Result};

/// A hash length the grammar can actually render: `1..=`[`MAX_HASH_LEN`].
///
/// Zero is not a length. A [`Base36Hash`](crate::Base36Hash) cannot be empty,
/// so `digest(_, 0)` has no value to return — before this type it panicked,
/// which is the worst thing a library can do across a foreign-function
/// boundary, where a Rust panic is a host-process abort rather than an
/// exception. Refining the length moves that failure to a [`Result`] the
/// caller handles once, at the edge, and makes every length *inside* the crate
/// total by construction.
///
/// ```
/// # use hashid::HashLength;
/// assert_eq!(HashLength::new(4)?.get(), 4);
/// assert!(HashLength::new(0).is_err());
/// assert!(HashLength::new(41).is_err());
/// # Ok::<(), hashid::IdError>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HashLength(usize);

impl HashLength {
    /// The shortest renderable hash: one character.
    pub const MIN: Self = Self(1);

    /// The longest hash the grammar permits, [`MAX_HASH_LEN`] characters.
    pub const MAX: Self = Self(MAX_HASH_LEN);

    /// The length `value`.
    ///
    /// # Errors
    ///
    /// [`IdError::InvalidHashLength`] unless `1 <= value <= MAX_HASH_LEN`.
    pub fn new(value: usize) -> Result<Self> {
        if value == 0 || value > MAX_HASH_LEN {
            return Err(IdError::InvalidHashLength {
                value,
                limit: MAX_HASH_LEN,
            });
        }
        Ok(Self(value))
    }

    /// `value`, clamped into `1..=MAX_HASH_LEN`.
    ///
    /// Crate-internal and deliberately total, so that compile-time constants
    /// and already-validated bounds need no `Result`. Caller-supplied lengths
    /// go through [`HashLength::new`], which refuses rather than substitutes.
    pub(crate) const fn clamped(value: usize) -> Self {
        if value < 1 {
            Self::MIN
        } else if value > MAX_HASH_LEN {
            Self::MAX
        } else {
            Self(value)
        }
    }

    /// The length as a count of characters.
    #[must_use]
    pub fn get(self) -> usize {
        self.0
    }

    /// One character wider, saturating at [`HashLength::MAX`].
    pub(crate) fn wider(self) -> Self {
        Self::clamped(self.0.saturating_add(1))
    }
}

impl fmt::Display for HashLength {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<HashLength> for usize {
    fn from(length: HashLength) -> Self {
        length.0
    }
}

impl TryFrom<usize> for HashLength {
    type Error = IdError;

    fn try_from(value: usize) -> Result<Self> {
        Self::new(value)
    }
}
