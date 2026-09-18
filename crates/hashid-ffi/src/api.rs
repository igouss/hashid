//! The free functions.
//!
//! # What is here, and what is not
//!
//! A binding earns its keep by exposing what you *cannot* write in your own
//! language. Everything here is a rule with real content: a grammar, a
//! normalization, a clock conversion that is easy to get subtly wrong.
//!
//! Everything derivable from `parse_id` in a line or two is deliberately
//! absent, because an export you have to look up is more expensive than the
//! line it saves. `Id` gives you `prefix`, `hash` and `childPath`, and from
//! those:
//!
//! ```text
//! depth        id.childPath.size
//! isRoot       id.childPath.isEmpty
//! parent       id.childPath.dropLast()   , re-joined with '.'
//! child(n)     "$id.$n"
//! descendant   a.prefix == b.prefix && a.hash == b.hash
//!                && a.childPath.size > b.childPath.size
//!                && a.childPath.startsWith(b.childPath)
//! ```

use hashid::{is_valid_id as core_is_valid_id, parse_id as core_parse_id, Prefix};

use crate::error::HashidError;
use crate::model::Id;

/// Parse `raw` into its validated parts.
///
/// The prefix may itself contain `-` (`bead-me-up-3e9` has prefix
/// `bead-me-up`), so the split point is the **last** `-`, not the first.
///
/// # Errors
///
/// `HashidError.Malformed`, carrying the precise rule that broke — which one,
/// not merely that something did, so a form can highlight one field.
#[uniffi::export]
pub fn parse_id(raw: String) -> Result<Id, HashidError> {
    Ok(Id::from(&core_parse_id(&raw)?))
}

/// Does `raw` satisfy the ID grammar?
///
/// The predicate form of `parse_id`, for when you do not need the reason.
#[uniffi::export]
#[must_use]
pub fn is_valid_id(raw: String) -> bool {
    core_is_valid_id(&raw)
}

/// Coerce arbitrary human input into a legal prefix.
///
/// Trims, lowercases ASCII, drops every character outside `[a-z0-9_-.:#]`,
/// clamps to 64 bytes, and strips trailing punctuation.
/// `  Project-Name_2!  ` becomes `project-name_2`.
///
/// `GeneratorConfig` takes its prefix **strictly**, so this is the step that
/// turns a human's answer into one. Nothing renames your namespace silently.
///
/// # Errors
///
/// `HashidError.UnusablePrefix` when no usable character survives. No
/// substitute is invented: a silent fallback would attach real IDs to a
/// namespace you never chose.
#[uniffi::export]
pub fn normalize_prefix(raw: String) -> Result<String, HashidError> {
    Ok(Prefix::normalize(&raw)?.as_str().to_owned())
}

/// The current instant, in nanoseconds since the Unix epoch, UTC.
///
/// Fills `Request.createdAtNanos`. It exists so that callers do not each
/// re-derive the conversion from their language's clock: that number is hashed
/// verbatim, and an implementation that rounds it to milliseconds mints a
/// different ID for the same record with nothing to warn you.
#[uniffi::export]
#[must_use]
pub fn now_nanos() -> i64 {
    chrono::Utc::now()
        .timestamp_nanos_opt()
        .unwrap_or_else(|| unreachable!("the system clock is outside 1677-09-21 ..= 2262-04-11"))
}
