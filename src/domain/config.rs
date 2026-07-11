//! The policy that shapes a minted ID.

use crate::domain::limits::MAX_HASH_LEN;
use crate::domain::prefix::Prefix;
use crate::error::{IdError, Result};

/// Default lower bound on hash length.
const DEFAULT_MIN_HASH_LEN: usize = 3;
/// Default upper bound on hash length.
const DEFAULT_MAX_HASH_LEN: usize = 8;
/// Default collision probability the adaptive sizer stays under.
const DEFAULT_MAX_COLLISION_PROB: f64 = 0.25;
/// Default prefix, used only by [`IdConfig::default`].
const DEFAULT_PREFIX: &str = "id";

/// How IDs should look: which namespace, and how much entropy to spend.
///
/// The hash length is not fixed — it is chosen per mint from the current
/// population, so a young dataset gets short, typeable IDs and a large one
/// automatically widens. `min`/`max` bracket that choice;
/// `max_collision_prob` is the birthday bound it stays under.
///
/// ```
/// # use hashid::{IdConfig, Prefix};
/// let config: IdConfig = IdConfig::new(Prefix::normalize("myproj")?)
///     .with_hash_lengths(4, 10)?;
/// assert_eq!(config.prefix().as_str(), "myproj");
/// # Ok::<(), hashid::IdError>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct IdConfig {
    prefix: Prefix,
    min_hash_length: usize,
    max_hash_length: usize,
    max_collision_prob: f64,
}

impl IdConfig {
    /// A config for `prefix`, with default entropy policy (3–8 base36
    /// characters, staying under a 25% birthday bound).
    #[must_use]
    pub fn new(prefix: Prefix) -> Self {
        Self {
            prefix,
            min_hash_length: DEFAULT_MIN_HASH_LEN,
            max_hash_length: DEFAULT_MAX_HASH_LEN,
            max_collision_prob: DEFAULT_MAX_COLLISION_PROB,
        }
    }

    /// Bracket the adaptive hash length.
    ///
    /// # Errors
    ///
    /// [`IdError::InvalidHashBounds`] unless `1 <= min <= max <= MAX_HASH_LEN`.
    pub fn with_hash_lengths(mut self, min: usize, max: usize) -> Result<Self> {
        if min == 0 || min > max || max > MAX_HASH_LEN {
            return Err(IdError::InvalidHashBounds {
                min,
                max,
                limit: MAX_HASH_LEN,
            });
        }
        self.min_hash_length = min;
        self.max_hash_length = max;
        Ok(self)
    }

    /// Set the birthday bound the adaptive sizer stays under.
    ///
    /// # Errors
    ///
    /// [`IdError::InvalidCollisionProb`] unless `0.0 < value < 1.0`.
    pub fn with_max_collision_prob(mut self, value: f64) -> Result<Self> {
        if !(value > 0.0 && value < 1.0) {
            return Err(IdError::InvalidCollisionProb { value });
        }
        self.max_collision_prob = value;
        Ok(self)
    }

    /// The namespace minted IDs carry.
    #[must_use]
    pub fn prefix(&self) -> &Prefix {
        &self.prefix
    }

    /// Lower bound on hash length.
    #[must_use]
    pub fn min_hash_length(&self) -> usize {
        self.min_hash_length
    }

    /// Upper bound on hash length.
    #[must_use]
    pub fn max_hash_length(&self) -> usize {
        self.max_hash_length
    }

    /// The birthday bound the adaptive sizer stays under.
    #[must_use]
    pub fn max_collision_prob(&self) -> f64 {
        self.max_collision_prob
    }
}

impl Default for IdConfig {
    /// Prefix `id`, hashes of 3–8 characters, 25% birthday bound.
    fn default() -> Self {
        let prefix: Prefix = Prefix::parse(DEFAULT_PREFIX)
            .unwrap_or_else(|_| unreachable!("`id` is a legal prefix"));
        Self::new(prefix)
    }
}
