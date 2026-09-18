//! A plain C ABI for `hashid`.
//!
//! Content-addressed, human-typeable short IDs — `br-8cda`,
//! `myproj-survey-the-users-3f9k1`, `br-8cda.1.2` — reachable from C, and from
//! every language that can call C: Java through JNA or the Foreign Function &
//! Memory API, Python through `ctypes`, Node through N-API or `ffi-napi`, Go
//! through cgo, C# through P/Invoke, Zig, Swift, and the rest.
//!
//! The header is [`include/hashid.h`](../../include/hashid.h). It is the
//! normative description of this ABI; this crate is its implementation.
//!
//! # The three rules
//!
//! 1. **Status codes.** Every function returns [`hashid_status`], and
//!    `HASHID_OK` is zero. On failure, out-parameters are left untouched and
//!    [`hashid_last_error_message`] describes what happened.
//! 2. **Ownership.** Every pointer this library returns is yours, and is
//!    released with the matching `hashid_*_free`. Every pointer you pass in is
//!    borrowed for the call and never retained. The one exception is
//!    [`hashid_last_error_message`], which is borrowed until your next call.
//! 3. **No unwinding.** Every entry point catches panics and reports
//!    `HASHID_ERR_PANIC`. A Rust panic crossing into C frames is undefined
//!    behaviour, so it never happens here.
//!
//! # You bring the store
//!
//! This library ships no store. [`hashid_contains_fn`] is the one question it
//! asks of yours — is this ID taken — and you pass it straight to
//! `hashid_minter_mint`. Your IDs already live somewhere; one callback over
//! that beats a second copy of it that can go stale.
//!
//! # How small this is
//!
//! Five free functions, three opaque handles, one vtable to fill in. Anything
//! you can derive from `hashid_parse_id` in a line — an ID's depth, its parent,
//! its n-th child — is left to you, because an export you have to look up costs
//! more than the line it saves.
//!
//! # Why this is a separate crate
//!
//! `hashid` itself keeps `unsafe_code = "forbid"` and three dependencies. This
//! crate holds every `unsafe extern "C"` boundary, so the domain never has to.
//! It is a driving adapter, and it contains no rules of its own — every
//! decision it appears to make is delegated to the core, on purpose: a boundary
//! that re-decides a domain rule is a boundary that will disagree with it.
//!
//! For Kotlin, Swift, Python, Ruby and Java via generated bindings, use
//! `hashid-ffi` instead, which is built on `UniFFI`. For a language with neither,
//! `spec/` specifies the format completely and ships conformance vectors.

// Every name here is spelled the way C spells it, so that the header and the
// implementation can be read side by side.
#![allow(non_camel_case_types)]

mod api;
mod convert;
mod minter;
mod memory;
mod status;
mod store;

pub use api::{
    hashid_is_valid_id, hashid_normalize_prefix, hashid_now_nanos,
    hashid_parse_id,
};
pub use convert::{hashid_config, hashid_request};
pub use minter::{
    hashid_minter, hashid_minter_free, hashid_minter_mint, hashid_minter_mint_unchecked,
    hashid_minter_mint_with_slug, hashid_minter_new, hashid_minter_with_config,
};
pub use memory::{hashid_id, hashid_id_free, hashid_string_free};
pub use status::{hashid_last_error_message, hashid_last_error_reason, hashid_status};
pub use store::hashid_contains_fn;

/// This library's version, e.g. `"0.1.0"`.
///
/// Borrowed and static: do not free it.
#[no_mangle]
pub extern "C" fn hashid_version() -> *const std::ffi::c_char {
    concat!(env!("CARGO_PKG_VERSION"), "\0")
        .as_ptr()
        .cast::<std::ffi::c_char>()
}

/// The version of the ID format specification this library implements, e.g.
/// `"1.0.0"`.
///
/// Borrowed and static: do not free it. Two libraries reporting the same spec
/// version mint identical IDs for identical content.
#[no_mangle]
pub extern "C" fn hashid_spec_version() -> *const std::ffi::c_char {
    concat!("1.0.0", "\0")
        .as_ptr()
        .cast::<std::ffi::c_char>()
}
