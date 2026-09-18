//! The one thing this crate needs from you: a lookup.
//!
//! Minting asks a single question — *is this ID already taken?* — and you
//! answer it with a closure. There is no trait to implement and no adapter to
//! write, because one question does not need an interface:
//!
//! ```
//! # use std::collections::HashSet;
//! # use hashid::{Id, Minter, Request};
//! # let db: HashSet<String> = HashSet::new();
//! let minter: Minter = Minter::new("myproj")?;
//! let id: Id = minter.mint(&Request::new("Ship the thing"), |id: &Id| {
//!     Ok(db.contains(&id.to_string()))
//! })?;
//! # Ok::<(), hashid::IdError>(())
//! ```
//!
//! Against a real store that closure is one line over a method you already
//! have — `|id| self.db.exists(id)` — and against a set it is the line above.

use core::fmt;
use std::error::Error;

/// Your lookup could not answer.
///
/// This type is why the lookup returns a `Result` rather than a `bool`. A
/// closure over a real store *can* fail, and if it could not say so it would
/// have to pick a lie: return `false` and invent a free ID that may already be
/// taken, or return `true` and burn a good candidate. Both are silent, and the
/// first corrupts data. The minter propagates this instead of guessing.
///
/// The crate does not care *why* — a dropped socket, a corrupt page, a
/// permissions error are all the same to it. It cares only that the answer is
/// genuinely unknown.
#[derive(Debug)]
pub struct StoreError {
    message: String,
    source: Option<Box<dyn Error + Send + Sync + 'static>>,
}

impl StoreError {
    /// A lookup failure described only by a message.
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            source: None,
        }
    }

    /// A lookup failure wrapping the underlying cause, preserving the chain.
    #[must_use]
    pub fn with_source(
        message: impl Into<String>,
        source: impl Error + Send + Sync + 'static,
    ) -> Self {
        Self {
            message: message.into(),
            source: Some(Box::new(source)),
        }
    }

    /// The human-readable description of the failure.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "id lookup failed: {}", self.message)
    }
}

impl Error for StoreError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.source
            .as_ref()
            .map(|boxed| -> &(dyn Error + 'static) { boxed.as_ref() })
    }
}
