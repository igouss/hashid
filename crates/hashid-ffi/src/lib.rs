//! `hashid` for every language `UniFFI` reaches.
//!
//! Content-addressed, human-typeable short IDs — `br-8cda`,
//! `myproj-survey-the-users-3f9k1`, `br-8cda.1.2` — callable from Kotlin, Java,
//! Swift, Python and Ruby, and from anything else a `UniFFI` backend exists for.
//!
//! # What crosses the boundary
//!
//! IDs cross as **strings**, not objects. A `br-8cda` is already the most
//! portable representation there is, every language can print and store one,
//! and the API re-parses on the way in, so an ID that reaches the core is
//! well-formed no matter what the caller assembled. `parse_id` takes one
//! apart when you need the pieces.
//!
//! Instants cross as **nanoseconds since the Unix epoch** (`now_nanos`),
//! because the number is hashed verbatim: two implementations that round it
//! differently mint different IDs for the same record.
//!
//! # You bring the store
//!
//! This binding ships no store. `IdStore` is the two questions it asks of
//! yours, and the only thing a mint or a resolution takes:
//!
//! ```text
//! class MyStore : IdStore {
//!     val ids = mutableSetOf<String>()
//!
//!     override fun contains(id: String) = id in ids
//!
//!     override fun findByHashFragment(fragment: String) =
//!         ids.filter { matchesFragment(it, fragment) }
//! }
//!
//! let store = MyStore()
//! let gen   = Generator(MinterConfig(prefix: "myproj"))
//!
//! let request = Request(title: "Ship the thing",
//!                         createdAtNanos: nowNanos(),
//!                         population: store.ids.size)
//! let id = gen.mint(request, store)     // "myproj-8cda"
//! store.ids.add(id)
//! ```
//!
//! The set above is a `Set` because the example is four lines long; yours is a
//! table. Either satisfies the interface, and `hashid` cannot tell which it got
//! — which is the reason there is no built-in store to graduate away from when
//! your ID set outgrows memory.
//!
//! Both methods are fallible on purpose. See `StoreError`.
//!
//! # How small this is
//!
//! Five free functions, two objects, one interface to implement. Anything you
//! can derive from `parse_id` in a line — an ID's depth, its parent, its n-th
//! child — is left to your language, because an export you have to look up
//! costs more than the line it saves. See the `api` module for the list and the
//! one-liners that replace it.
//!
//! # Architecture
//!
//! This crate is a boundary adapter and nothing else. Every rule lives in the
//! `hashid` crate, which has no `unsafe`, no I/O, and three dependencies; this
//! crate adds the `extern "C"` scaffolding `UniFFI` needs and translates types.
//! Keeping the two apart is what lets the core keep `unsafe_code = "forbid"`.
//!
//! For a language `UniFFI` does not reach, see `spec/` in the repository: the ID
//! format is fully specified and shipped with conformance vectors, and
//! `crates/hashid-c` exposes a plain C ABI.

#![allow(
    clippy::needless_pass_by_value,
    reason = "a `UniFFI`-exported signature owns its arguments: `&str` has no \
              representation on the far side of the boundary, so taking one \
              by reference is not an option the ABI offers"
)]

uniffi::setup_scaffolding!("hashid");

mod api;
mod error;
mod minter;
mod model;
mod num;
mod store;

pub use api::{is_valid_id, normalize_prefix, now_nanos, parse_id};
pub use error::{HashidError, MalformedReason, StoreError};
pub use minter::Minter;
pub use model::{MinterConfig, Id, Request};
pub use store::IdStore;
