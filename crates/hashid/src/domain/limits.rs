//! The hard bounds of the ID grammar.
//!
//! These are the only magic numbers in the crate, and they live in exactly one
//! place. Everything else derives its budget from them.

/// Longest permitted prefix, in bytes.
///
/// A prefix may itself contain hyphens (`bead-me-up-3e9` has prefix
/// `bead-me-up`), so this budget also has to cover an embedded slug.
pub const MAX_PREFIX_LEN: usize = 64;

/// Longest permitted base36 hash, in bytes.
pub const MAX_HASH_LEN: usize = 40;

/// Longest permitted slug, in bytes.
///
/// Capped well below [`MAX_PREFIX_LEN`] so that `<prefix>-<slug>` still fits
/// inside the prefix budget for any reasonable prefix.
pub const MAX_SLUG_LEN: usize = 48;

/// Longest permitted root ID (`<prefix>-<hash>`), in bytes.
///
/// Child paths (`.1.2`) extend beyond this; the bound covers the root form.
pub const MAX_ID_LEN: usize = MAX_PREFIX_LEN + 1 + MAX_HASH_LEN;
