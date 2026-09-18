//! Who owns what, and how it gets freed.
//!
//! Every pointer this library hands back is owned by the **caller** and must be
//! released with the matching `hashid_*_free` function. Every pointer the
//! caller hands in is borrowed for the duration of the call and is never
//! retained.
//!
//! There is exactly one exception, documented where it lives:
//! [`hashid_last_error_message`](crate::hashid_last_error_message) returns a
//! borrowed pointer valid only until the next call on that thread.

use std::ffi::{c_char, CString};
use std::ptr;

use crate::status::{fail, hashid_status};

/// A parsed identifier, taken apart.
///
/// Release with [`hashid_id_free`].
#[repr(C)]
#[derive(Debug)]
pub struct hashid_id {
    /// The canonical rendering, e.g. `br-8cda.1.2`.
    pub text: *mut c_char,
    /// The namespace half, e.g. `br`.
    pub prefix: *mut c_char,
    /// The uniquifying half: base36, e.g. `8cda`.
    pub hash: *mut c_char,
    /// `child_path_len` segments, or `NULL` for a root ID.
    pub child_path: *mut u32,
    /// How many child segments. Zero for a root ID.
    pub child_path_len: usize,
}

/// Allocate `value` as a caller-owned C string.
///
/// # Errors
///
/// Fails only if `value` contains an interior NUL, which nothing this library
/// produces ever does.
pub(crate) fn alloc_string(value: impl Into<Vec<u8>>) -> Result<*mut c_char, hashid_status> {
    // `impl Into<Vec<u8>>` rather than `&str`: an owned `String` moves straight
    // into the `CString` instead of being copied into a fresh allocation.
    CString::new(value).map(CString::into_raw).map_err(|_| {
        fail(
            hashid_status::HASHID_ERR_INVALID_UTF8,
            "internal value contained a NUL byte",
        )
    })
}

/// Allocate `segments` as a caller-owned `uint32_t` array.
pub(crate) fn alloc_u32_array(segments: &[u32]) -> (*mut u32, usize) {
    if segments.is_empty() {
        return (ptr::null_mut(), 0);
    }
    let boxed: Box<[u32]> = segments.to_vec().into_boxed_slice();
    let len: usize = boxed.len();
    (Box::into_raw(boxed).cast(), len)
}

/// Release a string previously returned by this library.
///
/// Passing `NULL` is a no-op. Passing anything this library did not allocate,
/// or the same pointer twice, is undefined behaviour.
fn free_string(value: *mut c_char) {
    if value.is_null() {
        return;
    }
    // SAFETY: the caller contract is that `value` came from `alloc_string`,
    // which produced it with `CString::into_raw`.
    drop(unsafe { CString::from_raw(value) });
}

/// Release a string previously returned by this library.
///
/// Passing `NULL` is a no-op.
///
/// # Safety
///
/// `value` must be a pointer this library returned and has not already been
/// freed.
#[no_mangle]
pub unsafe extern "C" fn hashid_string_free(value: *mut c_char) {
    free_string(value);
}

/// Release a parsed ID previously returned by this library.
///
/// Zeroes the struct, so a second call is harmless. Passing `NULL` is a no-op.
///
/// # Safety
///
/// `id` must point at a struct this library filled in and that has not already
/// been freed.
#[no_mangle]
pub unsafe extern "C" fn hashid_id_free(id: *mut hashid_id) {
    if id.is_null() {
        return;
    }
    // SAFETY: the caller contract is that `id` points at a valid, initialized
    // `hashid_id`.
    let id: &mut hashid_id = unsafe { &mut *id };
    free_string(id.text);
    free_string(id.prefix);
    free_string(id.hash);
    if !id.child_path.is_null() && id.child_path_len > 0 {
        // SAFETY: `child_path`/`child_path_len` came from `alloc_u32_array`.
        drop(unsafe {
            Box::from_raw(std::ptr::slice_from_raw_parts_mut(
                id.child_path,
                id.child_path_len,
            ))
        });
    }
    id.text = ptr::null_mut();
    id.prefix = ptr::null_mut();
    id.hash = ptr::null_mut();
    id.child_path = ptr::null_mut();
    id.child_path_len = 0;
}
