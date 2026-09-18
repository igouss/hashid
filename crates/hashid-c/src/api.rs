//! The free functions.
//!
//! # What is here, and what is not
//!
//! A binding earns its keep by exposing what you cannot write in C. Everything
//! here is a rule with real content: a grammar, a normalization, a clock
//! conversion that is easy to get subtly wrong.
//!
//! Everything derivable from `hashid_parse_id` in a line or two is deliberately
//! absent. `hashid_id` gives you `prefix`, `hash` and `child_path`, and from
//! those an ID's depth is `child_path_len`, a root is `child_path_len == 0`,
//! its n-th child is `sprintf("%s.%u", id, n)`, and its parent is the text up
//! to the last `.`.

use std::ffi::c_char;

use hashid::{
    is_valid_id as core_is_valid_id,
    parse_id as core_parse_id, Id, Prefix,
};

use crate::convert::borrow_str;
use crate::memory::{alloc_string, alloc_u32_array, hashid_id};
use crate::status::{fail, fail_with, guard, hashid_status, succeed, tri};

/// Parse `raw` into its validated parts, writing them to `out`.
///
/// The prefix may itself contain `-` (`bead-me-up-3e9` has prefix
/// `bead-me-up`), so the split point is the **last** `-`.
///
/// On success `out` is owned by the caller; release it with `hashid_id_free`.
/// On failure `out` is untouched.
///
/// # Safety
///
/// `raw` must be a valid NUL-terminated string and `out` a writable `hashid_id`.
#[no_mangle]
pub unsafe extern "C" fn hashid_parse_id(raw: *const c_char, out: *mut hashid_id) -> hashid_status {
    guard(|| {
        if out.is_null() {
            return fail(
                hashid_status::HASHID_ERR_NULL_ARGUMENT,
                "`out` must not be NULL",
            );
        }
        // SAFETY: the caller contract is that `raw` is NUL-terminated.
        let raw: &str = tri!(unsafe { borrow_str(raw, "raw") });
        let id: Id = match core_parse_id(raw) {
            Ok(id) => id,
            Err(err) => return fail_with(&err),
        };

        // Build every field before writing any of it: a partial `hashid_id`
        // would be one the caller must free and cannot inspect.
        let parts: [&str; 3] = [&id.to_string(), id.prefix().as_str(), id.hash().as_str()];
        let mut allocated: [*mut c_char; 3] = [std::ptr::null_mut(); 3];
        for (slot, text) in allocated.iter_mut().zip(parts) {
            match alloc_string(text) {
                Ok(pointer) => *slot = pointer,
                Err(status) => {
                    for done in allocated {
                        // SAFETY: each came from `alloc_string` and is unshared.
                        unsafe { crate::hashid_string_free(done) };
                    }
                    return status;
                }
            }
        }
        let (child_path, child_path_len): (*mut u32, usize) = alloc_u32_array(id.child_path());

        // SAFETY: the caller contract is that `out` is writable.
        unsafe {
            out.write(hashid_id {
                text: allocated[0],
                prefix: allocated[1],
                hash: allocated[2],
                child_path,
                child_path_len,
            });
        }
        succeed()
    })
}

/// Does `raw` satisfy the ID grammar?
///
/// The predicate form of `hashid_parse_id`, for when you do not need the
/// reason. `NULL` and non-UTF-8 are simply `false`; neither is a valid ID.
///
/// # Safety
///
/// `raw` must be NULL or a valid NUL-terminated string.
#[no_mangle]
pub unsafe extern "C" fn hashid_is_valid_id(raw: *const c_char) -> bool {
    guard(|| {
        // SAFETY: the caller contract is that `raw` is NULL or NUL-terminated.
        match unsafe { borrow_str(raw, "raw") } {
            Ok(raw) if core_is_valid_id(raw) => succeed(),
            Ok(_) | Err(_) => hashid_status::HASHID_ERR_MALFORMED,
        }
    }) == hashid_status::HASHID_OK
}

/// Coerce arbitrary human input into a legal prefix.
///
/// Trims, lowercases ASCII, drops every character outside `[a-z0-9_-.:#]`,
/// clamps to 64 bytes, and strips trailing punctuation. `  Project-Name_2!  `
/// becomes `project-name_2`.
///
/// `hashid_config` takes its prefix **strictly**, so this is the step that
/// turns a human's answer into one. Fails with `HASHID_ERR_UNUSABLE_PREFIX`
/// when nothing usable survives: no substitute is invented, because a silent
/// fallback would attach real IDs to a namespace you never chose.
///
/// Free `*out` with `hashid_string_free`.
///
/// # Safety
///
/// `raw` must be a valid NUL-terminated string and `out` writable.
#[no_mangle]
pub unsafe extern "C" fn hashid_normalize_prefix(
    raw: *const c_char,
    out: *mut *mut c_char,
) -> hashid_status {
    guard(|| {
        if out.is_null() {
            return fail(
                hashid_status::HASHID_ERR_NULL_ARGUMENT,
                "`out` must not be NULL",
            );
        }
        // SAFETY: the caller contract is that `raw` is NUL-terminated.
        let raw: &str = tri!(unsafe { borrow_str(raw, "raw") });
        let prefix: Prefix = match Prefix::normalize(raw) {
            Ok(prefix) => prefix,
            Err(err) => return fail_with(&err),
        };
        let allocated: *mut c_char = tri!(alloc_string(prefix.as_str()));
        // SAFETY: the caller contract is that `out` is writable.
        unsafe { out.write(allocated) };
        succeed()
    })
}
/// The current instant, in nanoseconds since the Unix epoch, UTC.
///
/// Fills `hashid_request.created_at_nanos`. It exists so that callers do not
/// each re-derive the conversion from their platform's clock: that number is
/// hashed verbatim, and an implementation that rounds it to milliseconds mints
/// a different ID for the same record with nothing to warn you.
#[no_mangle]
pub extern "C" fn hashid_now_nanos() -> i64 {
    chrono::Utc::now()
        .timestamp_nanos_opt()
        .unwrap_or_else(|| unreachable!("the system clock is outside 1677-09-21 ..= 2262-04-11"))
}
