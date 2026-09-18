//! Minting, as an opaque handle.

use std::ffi::{c_char, c_void};

use hashid::{HashLength, Id, IdConfig, Minter, Request};

use crate::convert::{
    borrow_config, borrow_request, borrow_str, hashid_config, hashid_request, narrow,
};
use crate::memory::alloc_string;
use crate::status::{fail, fail_with, guard, hashid_status, succeed, tri};
use crate::store::{ask, hashid_contains_fn};

/// A generator bound to one namespace and entropy policy.
///
/// Create with [`hashid_minter_new`], release with
/// [`hashid_minter_free`]. Immutable once created, and safe to share.
pub struct hashid_minter {
    inner: Minter,
}

/// A minter for `prefix`, with the default entropy policy.
///
/// The prefix is normalized the lenient way — trimmed, lowercased, illegal
/// characters dropped — because it usually comes from a human or a config
/// file. `  Project-Name_2!  ` becomes `project-name_2`. Fails with
/// `HASHID_ERR_UNUSABLE_PREFIX` when nothing usable survives: no substitute is
/// invented, because a silent fallback would attach real IDs to a namespace you
/// never chose.
///
/// # Safety
///
/// `prefix` must be a valid NUL-terminated string and `out` must be writable.
#[no_mangle]
pub unsafe extern "C" fn hashid_minter_new(
    prefix: *const c_char,
    out: *mut *mut hashid_minter,
) -> hashid_status {
    guard(|| {
        if out.is_null() {
            return fail(
                hashid_status::HASHID_ERR_NULL_ARGUMENT,
                "`out` must not be NULL",
            );
        }
        // SAFETY: the caller contract is that `prefix` is NUL-terminated.
        let prefix: &str = tri!(unsafe { borrow_str(prefix, "prefix") });
        let inner: Minter = match Minter::new(prefix) {
            Ok(minter) => minter,
            Err(err) => return fail_with(&err),
        };
        // SAFETY: the caller contract is that `out` is writable.
        unsafe { out.write(Box::into_raw(Box::new(hashid_minter { inner }))) };
        succeed()
    })
}

/// A minter bound to an explicit `config`, for a non-default entropy policy.
///
/// `config.prefix` is taken **strictly** — it must already be a legal prefix.
/// Run human input through `hashid_normalize_prefix` first, or use
/// [`hashid_minter_new`], which normalizes for you.
///
/// # Safety
///
/// `config` must point at a valid `hashid_config` and `out` must be writable.
#[no_mangle]
pub unsafe extern "C" fn hashid_minter_with_config(
    config: *const hashid_config,
    out: *mut *mut hashid_minter,
) -> hashid_status {
    guard(|| {
        if out.is_null() {
            return fail(
                hashid_status::HASHID_ERR_NULL_ARGUMENT,
                "`out` must not be NULL",
            );
        }
        // SAFETY: the caller contract covers `config`.
        let config: IdConfig = tri!(unsafe { borrow_config(config) });
        let built: Box<hashid_minter> = Box::new(hashid_minter {
            inner: Minter::with_config(config),
        });
        // SAFETY: the caller contract is that `out` is writable.
        unsafe { out.write(Box::into_raw(built)) };
        succeed()
    })
}

/// Release a minter handle. Passing `NULL` is a no-op.
///
/// # Safety
///
/// `minter` must come from [`hashid_minter_new`] or
/// [`hashid_minter_with_config`], must not already have been freed, and must
/// not be in use by another thread.
#[no_mangle]
pub unsafe extern "C" fn hashid_minter_free(minter: *mut hashid_minter) {
    if minter.is_null() {
        return;
    }
    // SAFETY: the caller contract is that it came from `Box::into_raw`.
    drop(unsafe { Box::from_raw(minter) });
}

/// Borrow a minter handle.
///
/// # Safety
///
/// `minter` must be NULL or a live handle.
unsafe fn borrow_minter<'a>(
    minter: *const hashid_minter,
) -> Result<&'a hashid_minter, hashid_status> {
    if minter.is_null() {
        return Err(fail(
            hashid_status::HASHID_ERR_NULL_ARGUMENT,
            "`minter` must not be NULL",
        ));
    }
    // SAFETY: delegated to this function's own contract.
    Ok(unsafe { &*minter })
}

/// Mint an ID at an explicit `nonce` and `hash_length`, **consulting no store**.
///
/// Purely deterministic: identical arguments yield an identical ID in every
/// language. This is the content-addressing primitive — use it to reproduce a
/// known ID, or to check an implementation against the conformance vectors. It
/// offers no uniqueness guarantee whatsoever; for that, call
/// [`hashid_minter_mint`].
///
/// # Safety
///
/// `generator` must be a live handle, `request` a valid `hashid_request`, and
/// `out` writable.
#[no_mangle]
pub unsafe extern "C" fn hashid_minter_mint_unchecked(
    minter: *const hashid_minter,
    request: *const hashid_request,
    nonce: u32,
    hash_length: u32,
    out: *mut *mut c_char,
) -> hashid_status {
    guard(|| {
        if out.is_null() {
            return fail(
                hashid_status::HASHID_ERR_NULL_ARGUMENT,
                "`out` must not be NULL",
            );
        }
        // SAFETY: the caller contract is that `generator` is live.
        let minter: &hashid_minter = tri!(unsafe { borrow_minter(minter) });
        // SAFETY: the caller contract covers `request`.
        let request: Request<'_> = tri!(unsafe { borrow_request(request) });
        let hash_length: HashLength =
            match HashLength::new(tri!(narrow(u64::from(hash_length), "hash_length"))) {
                Ok(length) => length,
                Err(err) => return fail_with(&err),
            };
        let id: Id = minter.inner.mint_unchecked(&request, nonce, hash_length);
        let allocated: *mut c_char = tri!(alloc_string(id.to_string()));
        // SAFETY: the caller contract is that `out` is writable.
        unsafe { out.write(allocated) };
        succeed()
    })
}

/// Mint an ID your store does not already hold.
///
/// Walks an escalation ladder, checking every candidate: ten nonces at the
/// adaptive length, then a wider hash, then a 12-character fallback, then the
/// fallback with the nonce spliced in.
///
/// Fails with `HASHID_ERR_EXHAUSTED` rather than return an ID it knows to be
/// taken — a duplicate ID is a silent data-corruption bug, an error is merely a
/// loud one — and with `HASHID_ERR_STORE` when your store could not answer.
///
/// # Safety
///
/// `minter` must be a live handle, `request` a valid `hashid_request`, `out`
/// writable, and `contains` callable with `user_data` for the duration of the
/// call.
#[no_mangle]
pub unsafe extern "C" fn hashid_minter_mint(
    minter: *const hashid_minter,
    request: *const hashid_request,
    contains: hashid_contains_fn,
    user_data: *mut c_void,
    out: *mut *mut c_char,
) -> hashid_status {
    guard(|| {
        if out.is_null() {
            return fail(
                hashid_status::HASHID_ERR_NULL_ARGUMENT,
                "`out` must not be NULL",
            );
        }
        // SAFETY: the caller contract is that the handle is live.
        let minter: &hashid_minter = tri!(unsafe { borrow_minter(minter) });
        // SAFETY: the caller contract covers `request`.
        let request: Request<'_> = tri!(unsafe { borrow_request(request) });

        let id: Id = match minter.inner.mint(&request, |id: &Id| ask(contains, user_data, id)) {
            Ok(id) => id,
            Err(err) => return fail_with(&err),
        };
        let allocated: *mut c_char = tri!(alloc_string(id.to_string()));
        // SAFETY: the caller contract is that `out` is writable.
        unsafe { out.write(allocated) };
        succeed()
    })
}

/// Mint an ID carrying a human-readable slug, e.g. `proj-survey-my-thing-8cda`.
///
/// The slug sits between prefix and hash, and the hash is still always
/// appended, because two records may share a slug. Falls back to a plain
/// hash-only ID when the slug normalizes to nothing, cannot fit the prefix
/// budget, or its namespace is saturated: producing a valid ID always beats
/// honouring a cosmetic request.
///
/// # Safety
///
/// As [`hashid_minter_mint`], plus `slug` being a valid NUL-terminated
/// string.
#[no_mangle]
pub unsafe extern "C" fn hashid_minter_mint_with_slug(
    minter: *const hashid_minter,
    request: *const hashid_request,
    slug: *const c_char,
    contains: hashid_contains_fn,
    user_data: *mut c_void,
    out: *mut *mut c_char,
) -> hashid_status {
    guard(|| {
        if out.is_null() {
            return fail(
                hashid_status::HASHID_ERR_NULL_ARGUMENT,
                "`out` must not be NULL",
            );
        }
        // SAFETY: the caller contract is that the handle is live.
        let minter: &hashid_minter = tri!(unsafe { borrow_minter(minter) });
        // SAFETY: the caller contract covers `request` and `slug`.
        let request: Request<'_> = tri!(unsafe { borrow_request(request) });
        // SAFETY: as above.
        let slug: &str = tri!(unsafe { borrow_str(slug, "slug") });

        let id: Id = match minter.inner.mint_with_slug(&request, slug, |id: &Id| ask(contains, user_data, id)) {
            Ok(id) => id,
            Err(err) => return fail_with(&err),
        };
        let allocated: *mut c_char = tri!(alloc_string(id.to_string()));
        // SAFETY: the caller contract is that `out` is writable.
        unsafe { out.write(allocated) };
        succeed()
    })
}
