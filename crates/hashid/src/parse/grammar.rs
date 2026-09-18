//! The ID grammar, and the only way to build an [`Id`] from text.
//!
//! ```text
//! id         := prefix "-" hash child-path?
//! prefix     := [a-z0-9_.:#-]{1,64}       ; may itself contain "-"
//! hash       := [a-z0-9]{1,40}            ; base36
//! child-path := ("." [0-9]+)+
//! ```
//!
//! The prefix may contain hyphens, so the split point is the **last** `-`, not
//! the first. That single decision is what lets `bead-me-up-3e9` keep the
//! prefix `bead-me-up`, and `external:jira-123` keep `external:jira`.

use core::str::FromStr;

use crate::domain::hash::Base36Hash;
use crate::domain::id::Id;
use crate::domain::prefix::Prefix;
use crate::error::{IdError, MalformedReason, Result};

/// Parse `raw` into a validated [`Id`].
///
/// # Errors
///
/// [`IdError::Malformed`], carrying the precise [`MalformedReason`] — which
/// rule broke, not merely that something did.
///
/// ```
/// # use hashid::{parse_id, MalformedReason, IdError};
/// let id = parse_id("bead-me-up-3e9")?;
/// assert_eq!(id.prefix().as_str(), "bead-me-up");
/// assert_eq!(id.hash().as_str(), "3e9");
///
/// let err = parse_id("bd-ABC").unwrap_err();
/// assert!(matches!(err, IdError::Malformed { reason: MalformedReason::HashCharset, .. }));
/// # Ok::<(), hashid::IdError>(())
/// ```
pub fn parse_id(raw: &str) -> Result<Id> {
    let Some(separator) = raw.rfind('-') else {
        return Err(IdError::malformed(raw, MalformedReason::MissingSeparator));
    };
    let (prefix_text, rest): (&str, &str) = raw.split_at(separator);
    let remainder: &str = &rest[1..];

    let prefix: Prefix = Prefix::parse(prefix_text).map_err(|err: IdError| reanchor(err, raw))?;

    let mut segments = remainder.split('.');
    let hash_text: &str = segments
        .next()
        .unwrap_or_else(|| unreachable!("split always yields at least one segment"));
    let hash: Base36Hash =
        Base36Hash::parse(hash_text).map_err(|err: IdError| reanchor(err, raw))?;

    let child_path: Vec<u32> = segments
        .map(|segment: &str| {
            segment
                .parse::<u32>()
                .map_err(|_| IdError::malformed(raw, MalformedReason::ChildSegment))
        })
        .collect::<Result<Vec<u32>>>()?;

    Ok(Id::from_parts(prefix, hash, child_path))
}

/// Does `raw` satisfy the ID grammar?
#[must_use]
pub fn is_valid_id(raw: &str) -> bool {
    parse_id(raw).is_ok()
}

/// Re-point a component-level error at the full ID the user actually typed.
///
/// `Prefix::parse` and `Base36Hash::parse` report the fragment they were given;
/// a caller who typed `bd-ABC` wants to see `bd-ABC`, not `ABC`.
fn reanchor(err: IdError, raw: &str) -> IdError {
    match err {
        IdError::Malformed { reason, .. } => IdError::malformed(raw, reason),
        other => other,
    }
}

impl Id {
    /// Parse `raw` into a validated [`Id`]. See [`parse_id`].
    ///
    /// # Errors
    ///
    /// [`IdError::Malformed`] carrying the specific [`MalformedReason`].
    pub fn parse(raw: &str) -> Result<Self> {
        parse_id(raw)
    }
}

impl FromStr for Id {
    type Err = IdError;

    fn from_str(raw: &str) -> Result<Self> {
        parse_id(raw)
    }
}
