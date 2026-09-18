//! The wire model: the records that cross the boundary.
//!
//! These types mirror the core's domain types without *being* them. That
//! separation is deliberate. `hashid::Id` is a value object whose invariants
//! are enforced by construction; a foreign `data class` has no such guarantee,
//! so the boundary carries plain data and re-parses on the way in. The
//! invariant is not weakened — it is re-established at every crossing, which is
//! the only thing a boundary can honestly promise.

use hashid::{
    Id as CoreId, IdConfig, Prefix, Request as CoreRequest,
};

use crate::error::HashidError;
use crate::num::{report, to_usize};

/// A parsed identifier, taken apart.
///
/// Returned by `parse_id`. Every field is already validated;
/// to go the other way, hand `Id::text` — or the parts, via
/// `format_id` — back to the API, which re-parses.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct Id {
    /// The canonical rendering, e.g. `br-8cda.1.2`.
    pub text: String,
    /// The namespace half, e.g. `br`. May itself contain `-` and an embedded slug.
    pub prefix: String,
    /// The uniquifying half: base36, e.g. `8cda`.
    pub hash: String,
    /// The hierarchy path, empty for a root ID. `br-8cda.1.2` gives `[1, 2]`.
    pub child_path: Vec<i64>,
}

impl From<&CoreId> for Id {
    fn from(id: &CoreId) -> Self {
        Self {
            text: id.to_string(),
            prefix: id.prefix().as_str().to_owned(),
            hash: id.hash().as_str().to_owned(),
            child_path: id.child_path().iter().copied().map(i64::from).collect(),
        }
    }
}

/// The content an ID is derived from, plus the context needed to size it.
///
/// IDs are content-addressed: the same `title`, `description`, `creator`,
/// `created_at_nanos` and nonce always produce the same ID, in every language.
/// In practice `created_at_nanos` supplies the entropy that keeps two
/// identically-titled records apart.
#[derive(Debug, Clone, uniffi::Record)]
pub struct Request {
    /// The primary content field. Always hashed.
    pub title: String,
    /// Optional body. Hashed as an empty string when absent.
    #[uniffi(default = None)]
    pub description: Option<String>,
    /// Optional author. Hashed as an empty string when absent.
    #[uniffi(default = None)]
    pub creator: Option<String>,
    /// Creation instant, in nanoseconds since the Unix epoch, UTC.
    ///
    /// Nanoseconds, not a language-native date type, because this number is
    /// hashed verbatim: two implementations that round it differently mint
    /// different IDs for the same record. `now_nanos`
    /// produces one for the current instant.
    pub created_at_nanos: i64,
    /// Roughly how many IDs already exist. Picks the starting hash length.
    ///
    /// Defaults to zero, which starts the hash at the shortest length the
    /// config allows. That cannot mint a duplicate — every candidate is still
    /// checked against your store, and the ladder widens the hash as it
    /// collides — it only costs extra lookups on a large dataset. An estimate
    /// is entirely adequate: being out by a factor of two moves the length by
    /// at most one character, so a cached or approximate row count is the right
    /// thing to pass, not a `select count(*)` per mint.
    #[uniffi(default = 0)]
    pub population: i64,
}

impl Request {
    /// Borrow this request as the core's request type.
    ///
    /// # Errors
    ///
    /// `HashidError::OutOfRange` when `population` is negative. The instant
    /// needs no validation: it already crossed as the nanoseconds the core
    /// hashes, so there is no date type to convert and nothing to reject.
    pub(crate) fn as_core(&self) -> Result<CoreRequest<'_>, HashidError> {
        let mut request: CoreRequest<'_> = CoreRequest::new(&self.title)
            .at_nanos(self.created_at_nanos)
            .with_population(to_usize("population", self.population)?);
        if let Some(description) = self.description.as_deref() {
            request = request.with_description(description);
        }
        if let Some(creator) = self.creator.as_deref() {
            request = request.with_creator(creator);
        }
        Ok(request)
    }
}

/// How IDs should look: which namespace, and how much entropy to spend.
///
/// `prefix` is taken **strictly** — it must already be a legal prefix. Pass
/// human or config input through `normalize_prefix`
/// first. Nothing here silently renames your namespace.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct MinterConfig {
    /// The namespace minted IDs carry, e.g. `br`.
    pub prefix: String,
    /// Lower bound on the adaptive hash length.
    #[uniffi(default = 3)]
    pub min_hash_length: i64,
    /// Upper bound on the adaptive hash length.
    #[uniffi(default = 8)]
    pub max_hash_length: i64,
    /// The birthday bound the adaptive sizer stays under.
    #[uniffi(default = 0.25)]
    pub max_collision_prob: f64,
}

impl MinterConfig {
    /// Validate into the core's config.
    pub(crate) fn to_core(&self) -> Result<IdConfig, HashidError> {
        let prefix: Prefix = Prefix::parse(&self.prefix)?;
        Ok(IdConfig::new(prefix)
            .with_hash_lengths(
                to_usize("minHashLength", self.min_hash_length)?,
                to_usize("maxHashLength", self.max_hash_length)?,
            )?
            .with_max_collision_prob(self.max_collision_prob)?)
    }
}

impl From<&IdConfig> for MinterConfig {
    fn from(config: &IdConfig) -> Self {
        Self {
            prefix: config.prefix().as_str().to_owned(),
            min_hash_length: report(config.min_hash_length().get()),
            max_hash_length: report(config.max_hash_length().get()),
            max_collision_prob: config.max_collision_prob(),
        }
    }
}
