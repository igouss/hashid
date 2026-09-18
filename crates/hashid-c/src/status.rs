//! Status codes, the thread-local last error, and the panic fence.
//!
//! Three rules hold for every exported function in this crate:
//!
//! 1. It returns a [`hashid_status`], and `HASHID_OK` is zero.
//! 2. On failure it records a human-readable message retrievable by
//!    [`hashid_last_error_message`], and leaves every out-parameter untouched.
//! 3. It cannot unwind. A Rust panic crossing into C is undefined behaviour, so
//!    every entry point runs inside [`guard`], which converts a panic into
//!    `HASHID_ERR_PANIC`.

use std::cell::RefCell;
use std::ffi::CString;
use std::panic::{catch_unwind, AssertUnwindSafe};

use hashid::{IdError, MalformedReason};

/// The outcome of a `hashid_*` call.
///
/// Zero is success; every other value is a failure with a message available
/// from [`hashid_last_error_message`].
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum hashid_status {
    /// The call succeeded.
    HASHID_OK = 0,
    /// An input does not satisfy the ID grammar.
    HASHID_ERR_MALFORMED = 1,
    /// An ID carries a prefix the caller does not accept.
    HASHID_ERR_PREFIX_MISMATCH = 2,
    /// A fragment matched more than one known ID.
    HASHID_ERR_AMBIGUOUS = 3,
    /// A fragment matched nothing.
    HASHID_ERR_NOT_FOUND = 4,
    /// A prefix normalized away to nothing.
    HASHID_ERR_UNUSABLE_PREFIX = 5,
    /// Hash-length bounds are not a satisfiable range.
    HASHID_ERR_INVALID_HASH_BOUNDS = 6,
    /// A single hash length is outside `1..=40`.
    HASHID_ERR_INVALID_HASH_LENGTH = 7,
    /// The target collision probability is not a probability.
    HASHID_ERR_INVALID_COLLISION_PROB = 8,
    /// The generator ran out of candidates without finding a free ID.
    HASHID_ERR_EXHAUSTED = 9,
    /// The caller's store failed to answer a lookup.
    HASHID_ERR_STORE = 10,
    /// A required pointer argument was NULL.
    HASHID_ERR_NULL_ARGUMENT = 11,
    /// A string argument was not valid UTF-8.
    HASHID_ERR_INVALID_UTF8 = 12,
    /// A panic was caught at the boundary. Treat as a bug report.
    HASHID_ERR_PANIC = 13,
    /// A failure this version of the ABI has no name for.
    ///
    /// The core's error type is `#[non_exhaustive]`, so it may grow a variant
    /// this header predates. Reporting that as something it is not — a
    /// malformed input, say — would send a caller looking in the wrong place;
    /// `hashid_last_error_message` still carries the real description.
    HASHID_ERR_UNKNOWN = 14,
    /// A number in a caller's struct is outside the range its field allows.
    ///
    /// Reported rather than clamped. A saturating conversion turns a mistake
    /// into a plausible number and tells the caller nothing.
    HASHID_ERR_OUT_OF_RANGE = 15,
}

thread_local! {
    /// The most recent failure on this thread, kept alive until the next one.
    static LAST_ERROR: RefCell<Option<CString>> = const { RefCell::new(None) };
}

/// Fallible bodies read better with an early return than with nested matches.
macro_rules! tri {
    ($expr:expr) => {
        match $expr {
            Ok(value) => value,
            Err(status) => return status,
        }
    };
}
pub(crate) use tri;

/// Record `message` as this thread's last error and return `status`.
///
/// Clears the `Malformed` reason; [`fail_with`] sets it where there is one.
pub(crate) fn fail(status: hashid_status, message: impl Into<String>) -> hashid_status {
    set_reason(0);
    let message: String = message.into();
    // A NUL inside the message would truncate it; replace rather than lose the
    // tail, since this string exists only to be read by a human.
    let sanitized: String = message.replace('\0', "\u{fffd}");
    let encoded: Option<CString> = CString::new(sanitized).ok();
    LAST_ERROR.with(|slot: &RefCell<Option<CString>>| {
        *slot.borrow_mut() = encoded;
    });
    status
}

/// Clear this thread's last error and return `HASHID_OK`.
pub(crate) fn succeed() -> hashid_status {
    LAST_ERROR.with(|slot: &RefCell<Option<CString>>| {
        *slot.borrow_mut() = None;
    });
    set_reason(0);
    hashid_status::HASHID_OK
}

/// The status a given [`IdError`] maps to.
fn status_for(err: &IdError) -> hashid_status {
    match err {
        IdError::Malformed { .. } => hashid_status::HASHID_ERR_MALFORMED,
        IdError::PrefixMismatch { .. } => hashid_status::HASHID_ERR_PREFIX_MISMATCH,
        IdError::UnusablePrefix { .. } => hashid_status::HASHID_ERR_UNUSABLE_PREFIX,
        IdError::InvalidHashBounds { .. } => hashid_status::HASHID_ERR_INVALID_HASH_BOUNDS,
        IdError::InvalidHashLength { .. } => hashid_status::HASHID_ERR_INVALID_HASH_LENGTH,
        IdError::InvalidCollisionProb { .. } => hashid_status::HASHID_ERR_INVALID_COLLISION_PROB,
        IdError::Exhausted { .. } => hashid_status::HASHID_ERR_EXHAUSTED,
        IdError::Store(_) => hashid_status::HASHID_ERR_STORE,
        IdError::TimestampOutOfRange { .. } => hashid_status::HASHID_ERR_OUT_OF_RANGE,
        // `IdError` is `#[non_exhaustive]`: a variant added upstream lands here
        // rather than breaking the build, and still reports its own message.
        _ => hashid_status::HASHID_ERR_UNKNOWN,
    }
}

/// Record `err` — message and, for a `Malformed`, its reason — and return the
/// status it maps to.
pub(crate) fn fail_with(err: &IdError) -> hashid_status {
    let status: hashid_status = fail(status_for(err), err.to_string());
    if let IdError::Malformed { reason, .. } = err {
        set_reason(reason_code(*reason));
    }
    status
}

/// Which grammar rule a `Malformed` error names, as a stable integer.
///
/// Exposed so a C caller can branch on the reason without parsing English.
fn reason_code(reason: MalformedReason) -> i32 {
    match reason {
        MalformedReason::Empty => 1,
        MalformedReason::MissingSeparator => 2,
        MalformedReason::EmptyPrefix => 3,
        MalformedReason::PrefixTooLong => 4,
        MalformedReason::PrefixCharset => 5,
        MalformedReason::EmptyHash => 6,
        MalformedReason::HashTooLong => 7,
        MalformedReason::HashCharset => 8,
        MalformedReason::ChildSegment => 9,
        _ => 0,
    }
}

/// Run `body` with a panic fence around it.
///
/// A Rust panic unwinding into C frames is undefined behaviour. Every exported
/// function is wrapped in this, so the worst a bug in this crate can do to a
/// host process is return `HASHID_ERR_PANIC`.
pub(crate) fn guard(body: impl FnOnce() -> hashid_status) -> hashid_status {
    match catch_unwind(AssertUnwindSafe(body)) {
        Ok(status) => status,
        Err(_) => fail(
            hashid_status::HASHID_ERR_PANIC,
            "hashid panicked; this is a bug, please report it",
        ),
    }
}

/// A human-readable description of the most recent failure **on this thread**.
///
/// Returns `NULL` when the last call on this thread succeeded.
///
/// The returned pointer is owned by the library and stays valid only until the
/// next `hashid_*` call on this thread. Copy it if you need to keep it. Do not
/// free it.
#[no_mangle]
pub extern "C" fn hashid_last_error_message() -> *const std::os::raw::c_char {
    LAST_ERROR.with(|slot: &RefCell<Option<CString>>| {
        slot.borrow()
            .as_ref()
            .map_or(std::ptr::null(), |message: &CString| message.as_ptr())
    })
}

/// The `Malformed` reason code of the most recent failure, or `0` when the last
/// failure was not a `Malformed` error.
///
/// Codes: 1 `Empty`, 2 `MissingSeparator`, 3 `EmptyPrefix`, 4 `PrefixTooLong`,
/// 5 `PrefixCharset`, 6 `EmptyHash`, 7 `HashTooLong`, 8 `HashCharset`,
/// 9 `ChildSegment`. A code this list does not name is a rule newer than your
/// header; treat it as "malformed, reason unknown".
#[no_mangle]
pub extern "C" fn hashid_last_error_reason() -> i32 {
    LAST_REASON.with(|slot: &std::cell::Cell<i32>| slot.get())
}

thread_local! {
    /// The `Malformed` reason accompanying [`LAST_ERROR`], or zero.
    static LAST_REASON: std::cell::Cell<i32> = const { std::cell::Cell::new(0) };
}

/// Record the reason that goes with the error just stored.
fn set_reason(reason: i32) {
    LAST_REASON.with(|slot: &std::cell::Cell<i32>| slot.set(reason));
}
