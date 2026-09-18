//! Integer widths, reconciled across the boundary.
//!
//! The core counts in `usize`; no foreign ABI has one. Every crossing goes
//! through a function here, so the rules live in one file rather than being
//! re-decided at each call site.
//!
//! # Nothing here clamps
//!
//! A conversion that saturates turns a caller's mistake into a plausible
//! number: a negative population becomes zero, a nonce of `-1` becomes four
//! billion, and the caller is told nothing. That is the failure this library
//! exists to prevent, one layer down — so every narrowing that can lose a
//! value returns [`HashidError::OutOfRange`] instead, and every widening that
//! cannot is written as a total function.
//!
//! # Why everything crossing is signed
//!
//! Rust's `u32` and `u64` render as Kotlin's `UInt` and `ULong`, which are
//! inline value classes. The JVM mangles the name of any method mentioning
//! one, so `mintUnchecked` would not be callable from Java at all, and
//! `Request` would not be constructible. Nothing this binding exposes can
//! reach `i64::MAX`, so signing costs a range nobody can use.

use crate::error::HashidError;

/// Widen a count the core produced. Total: every `u32` is an `i64`.
pub(crate) fn widen(value: u32) -> i64 {
    i64::from(value)
}

/// A caller's `i64` as a `usize`, refusing what it cannot mean.
///
/// # Errors
///
/// [`HashidError::OutOfRange`] when `value` is negative, or larger than this
/// platform's `usize`.
pub(crate) fn to_usize(field: &str, value: i64) -> Result<usize, HashidError> {
    usize::try_from(value).map_err(|_| HashidError::OutOfRange {
        field: field.to_owned(),
        value,
    })
}

/// A caller's `i64` as a `u32`, refusing what it cannot mean.
///
/// # Errors
///
/// [`HashidError::OutOfRange`] when `value` is negative or exceeds `u32::MAX`.
pub(crate) fn to_u32(field: &str, value: i64) -> Result<u32, HashidError> {
    u32::try_from(value).map_err(|_| HashidError::OutOfRange {
        field: field.to_owned(),
        value,
    })
}

/// A core-produced `usize` on its way into an error message, as an `i64`.
///
/// Total in practice: every value reported this way is a hash length, capped
/// at 40 by the grammar, or a number that reached the core as an `i64` in the
/// first place. Said loudly rather than clamped, because a clamped limit in an
/// error message is a number that looks right and is not.
pub(crate) fn report(value: usize) -> i64 {
    i64::try_from(value)
        .unwrap_or_else(|_| unreachable!("a reported bound cannot exceed `i64::MAX`"))
}
