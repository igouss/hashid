//! The one thing this library needs from you: a lookup.
//!
//! Minting asks a single question — *is this ID already taken?* — and you
//! answer it with a function pointer. There is no store handle to create and
//! free, and no vtable to fill in, because one question does not need an
//! interface:
//!
//! ```c
//! static int32_t known_contains(void *ctx, const char *id, const char **err) {
//!     switch (db_lookup(ctx, id)) {
//!         case FOUND:     return 1;
//!         case NOT_FOUND: return 0;
//!         default:        *err = "database unreachable"; return -1;
//!     }
//! }
//!
//! hashid_minter_mint(minter, &request, known_contains, my_db, &id);
//! ```
//!
//! There is no built-in store, and that is deliberate: a store shipped with a
//! library is a hash set that ends up in production holding a set which stopped
//! fitting in memory two releases ago. Your IDs already live somewhere — an
//! array, a sqlite table, an index — and one callback over that is both less
//! code than a second copy of it and the only version that cannot go stale.
//!
//! The callback may **fail**, and that is load-bearing. If it could not say "I
//! do not know", an implementation talking to a real database would have to
//! pick a lie when the connection drops: answer "not taken" and invent an ID
//! that may already exist, or answer "taken" and burn a good candidate. Both
//! are silent, and the first corrupts data. Returning a negative value is the
//! third option, and the minter propagates it rather than guessing.

use std::ffi::{c_char, c_void, CStr, CString};

use hashid::{Id, StoreError};

/// Is `id` already taken?
///
/// Return `1` for taken, `0` for free, and anything negative for "I could not
/// find out". On failure you may set `*error` to a NUL-terminated string
/// describing why; the library copies it before returning, so you keep
/// ownership.
///
/// `user_data` is whatever you passed alongside this pointer, handed straight
/// back. It may be `NULL`.
pub type hashid_contains_fn = unsafe extern "C" fn(
    user_data: *mut c_void,
    id: *const c_char,
    error: *mut *const c_char,
) -> i32;

/// Ask the caller's callback about one candidate.
///
/// Rendering the [`Id`] to text here, in one place, is what keeps the minting
/// entry points free of the conversion.
pub(crate) fn ask(
    contains: hashid_contains_fn,
    user_data: *mut c_void,
    id: &Id,
) -> Result<bool, StoreError> {
    let rendered: String = id.to_string();
    let encoded: CString = CString::new(rendered.as_str())
        .map_err(|_| StoreError::new(format!("`{rendered}` contains a NUL byte")))?;
    let mut error: *const c_char = std::ptr::null();

    // SAFETY: both pointers are valid for the duration of the call, and the
    // caller's contract is that `contains` is callable with `user_data`.
    let answer: i32 = unsafe { contains(user_data, encoded.as_ptr(), &raw mut error) };
    if answer < 0 {
        return Err(StoreError::new(message(error)));
    }
    Ok(answer > 0)
}

/// Read the callback's optional message, falling back to a generic one.
fn message(error: *const c_char) -> String {
    const FALLBACK: &str = "store `contains` callback failed";
    if error.is_null() {
        return FALLBACK.to_owned();
    }
    // SAFETY: the callback contract is that `error` is NULL or a valid
    // NUL-terminated string it keeps alive for the call.
    unsafe { CStr::from_ptr(error) }
        .to_str()
        .map_or_else(|_| FALLBACK.to_owned(), ToOwned::to_owned)
}
