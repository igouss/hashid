//! Checking an ID belongs to a namespace you accept.

use crate::domain::id::Id;
use crate::domain::prefix::Prefix;
use crate::error::{IdError, Result};

/// Confirm `id` carries `expected`, or one of the `also_allowed` prefixes.
///
/// The allow-list exists for imports: a dataset merged in from another project
/// keeps its own prefix, and you want to accept it without accepting everything.
///
/// # Errors
///
/// [`IdError::PrefixMismatch`] when the ID's prefix is neither `expected` nor
/// in `also_allowed`.
///
/// ```
/// # use hashid::{parse_id, validate_prefix, Prefix};
/// let id = parse_id("bd-abc123")?;
/// let bd: Prefix = Prefix::parse("bd")?;
/// let other: Prefix = Prefix::parse("other")?;
///
/// assert!(validate_prefix(&id, &bd, &[]).is_ok());
/// assert!(validate_prefix(&id, &other, &[bd.clone()]).is_ok());
/// assert!(validate_prefix(&id, &other, &[]).is_err());
/// # Ok::<(), hashid::IdError>(())
/// ```
pub fn validate_prefix(id: &Id, expected: &Prefix, also_allowed: &[Prefix]) -> Result<()> {
    if id.prefix() == expected || also_allowed.contains(id.prefix()) {
        return Ok(());
    }
    Err(IdError::PrefixMismatch {
        expected: expected.as_str().to_owned(),
        found: id.prefix().as_str().to_owned(),
    })
}
