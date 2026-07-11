//! What the caller wants an ID *for*.

use chrono::{DateTime, Utc};

/// The content an ID is derived from, plus the context needed to size it.
///
/// IDs are content-addressed: the same `title`, `description`, `creator`,
/// `created_at`, and nonce always produce the same ID. In practice
/// `created_at` supplies the entropy that keeps two identically-titled records
/// apart.
///
/// `population` — how many IDs already exist — is a **required** constructor
/// argument, not a defaulted field. It drives the adaptive hash length, and a
/// caller who forgets it silently mints minimum-entropy IDs into a large
/// dataset. Making it positional means you cannot forget it.
///
/// ```
/// # use chrono::Utc;
/// # use hashid::IdRequest;
/// let request: IdRequest = IdRequest::new("Fix the flaky test", Utc::now(), 1_200)
///     .with_description("It fails on CI about one run in five.")
///     .with_creator("iouri");
/// assert_eq!(request.population, 1_200);
/// ```
#[derive(Debug, Clone, Copy)]
pub struct IdRequest<'a> {
    /// The primary content field. Always hashed.
    pub title: &'a str,
    /// Optional body. Hashed as an empty string when absent.
    pub description: Option<&'a str>,
    /// Optional author. Hashed as an empty string when absent.
    pub creator: Option<&'a str>,
    /// The instant the record was created. The de facto entropy source.
    pub created_at: DateTime<Utc>,
    /// How many IDs already exist. Drives the adaptive hash length.
    pub population: usize,
}

impl<'a> IdRequest<'a> {
    /// A request for `title`, created at `created_at`, against an existing
    /// population of `population` IDs.
    #[must_use]
    pub fn new(title: &'a str, created_at: DateTime<Utc>, population: usize) -> Self {
        Self {
            title,
            description: None,
            creator: None,
            created_at,
            population,
        }
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
