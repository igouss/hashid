//! What the caller wants an ID *for*.

use chrono::{DateTime, Utc};

use crate::error::{IdError, Result};

/// An instant as nanoseconds since the Unix epoch, or `None` if it has no
/// such representation.
fn nanos_of(instant: DateTime<Utc>) -> Option<i64> {
    instant.timestamp_nanos_opt()
}

/// The content an ID is derived from.
///
/// IDs are content-addressed: the same `title`, `description`, `creator`,
/// `created_at` and nonce always produce the same ID. In practice `created_at`
/// supplies the entropy that keeps two identically-titled records apart, which
/// is why it defaults to *now* and why two records created in the same
/// nanosecond with the same content are the case the escalation ladder exists
/// for.
///
/// ```
/// # use hashid::Request;
/// let request: Request = Request::new("Fix the flaky test")
///     .with_description("It fails on CI about one run in five.")
///     .with_creator("iouri")
///     .with_population(1_200);
///
/// assert!(request.created_at_nanos() > 0);
/// ```
#[derive(Debug, Clone, Copy)]
pub struct Request<'a> {
    /// The primary content field. Always hashed.
    pub title: &'a str,
    /// Optional body. Hashed as an empty string when absent.
    pub description: Option<&'a str>,
    /// Optional author. Hashed as an empty string when absent.
    pub creator: Option<&'a str>,
    /// The instant the record was created, as nanoseconds since the Unix
    /// epoch. The de facto entropy source.
    ///
    /// Private, and validated by every constructor, because this number is
    /// hashed verbatim. A `DateTime<Utc>` can hold instants that have no
    /// nanosecond representation — before 1677 or after 2262 — and silently
    /// hashing those as zero would mint one colliding ID for all of them.
    created_at_nanos: i64,
    /// Roughly how many IDs already exist. Picks the starting hash length.
    pub population: usize,
}

impl<'a> Request<'a> {
    /// A request for `title`, created now, against an unknown population.
    ///
    /// The two defaults are safe in different ways. `created_at` is *now*,
    /// which is what a record being created wants; pass [`Request::at`] when
    /// reproducing a known ID instead.
    ///
    /// `population` defaults to zero, which starts the hash at the shortest
    /// length the config allows. That cannot mint a duplicate — every
    /// candidate is still checked, and the ladder widens the hash as it
    /// collides — it only costs extra lookups on a large dataset. Pass
    /// [`Request::with_population`] to skip straight to the right width; an
    /// estimate is entirely adequate, since being out by a factor of two moves
    /// the length by at most one character.
    #[must_use]
    pub fn new(title: &'a str) -> Self {
        Self {
            title,
            description: None,
            creator: None,
            created_at_nanos: nanos_of(Utc::now()).unwrap_or_else(|| {
                unreachable!("the system clock is outside 1677-09-21 ..= 2262-04-11")
            }),
            population: 0,
        }
    }

    /// Pin the creation instant, rather than taking now.
    ///
    /// The instant is hashed verbatim, so this is what makes an ID
    /// reproducible: the same content at the same nanosecond mints the same
    /// ID in every implementation.
    ///
    /// # Errors
    ///
    /// [`IdError::TimestampOutOfRange`] when `created_at` has no nanosecond
    /// representation — before 1677-09-21 or after 2262-04-11. Refused rather
    /// than coerced: a silent fallback would hash one value for every
    /// out-of-range instant, minting a single colliding ID for all of them.
    pub fn at(mut self, created_at: DateTime<Utc>) -> Result<Self> {
        self.created_at_nanos =
            nanos_of(created_at).ok_or_else(|| IdError::TimestampOutOfRange {
                timestamp: created_at.to_rfc3339(),
            })?;
        Ok(self)
    }

    /// Pin the creation instant as nanoseconds since the Unix epoch.
    ///
    /// Total, because every `i64` is a representable instant. This is what a
    /// boundary hands over: the number crosses verbatim, with no date type to
    /// convert and nothing to reject.
    #[must_use]
    pub fn at_nanos(mut self, created_at_nanos: i64) -> Self {
        self.created_at_nanos = created_at_nanos;
        self
    }

    /// The creation instant, as the nanoseconds that get hashed.
    #[must_use]
    pub fn created_at_nanos(&self) -> i64 {
        self.created_at_nanos
    }

    /// Roughly how many IDs already exist, so the hash starts wide enough.
    #[must_use]
    pub fn with_population(mut self, population: usize) -> Self {
        self.population = population;
        self
    }

    /// Attach a description to hash.
    #[must_use]
    pub fn with_description(mut self, description: &'a str) -> Self {
        self.description = Some(description);
        self
    }

    /// Attach a creator to hash.
    #[must_use]
    pub fn with_creator(mut self, creator: &'a str) -> Self {
        self.creator = Some(creator);
        self
    }
}
