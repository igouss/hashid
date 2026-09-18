//! Content-addressed, human-typeable short IDs.
//!
//! `hashid` mints identifiers like `br-8cda` or `myproj-survey-the-users-3f9k1`
//! — short enough to retype from a terminal, stable enough to put in a commit
//! message, and unique enough to key a database on.
//!
//! # Minting
//!
//! Two lines and a lookup. You bring the store; the crate asks it one question.
//!
//! ```
//! use std::collections::HashSet;
//! use hashid::{Id, Minter, Request};
//!
//! let known: HashSet<String> = HashSet::new();
//! let minter: Minter = Minter::new("myproj")?;
//!
//! let id: Id = minter.mint(&Request::new("Ship the thing"), |id: &Id| {
//!     Ok(known.contains(&id.to_string()))
//! })?;
//!
//! assert!(id.to_string().starts_with("myproj-"));   // myproj-8cda
//! # Ok::<(), hashid::IdError>(())
//! ```
//!
//! Against a real store the closure is one line over a method you already have:
//!
//! ```text
//! minter.mint(&request, |id| db.id_exists(&id.to_string()))?
//! ```
//!
//! There is no trait to implement and no adapter to write, because one question
//! does not need an interface. The closure returns `Result` on purpose — see
//! [`StoreError`].
//!
//! # How an ID is built
//!
//! ```text
//! br - survey-the-users - 3f9k1 . 2
//! ^^   ^^^^^^^^^^^^^^^^   ^^^^^   ^
//! │    │                  │       └─ child path (optional, hierarchical)
//! │    │                  └───────── base36 hash, adaptively sized
//! │    └──────────────────────────── slug (optional, human-readable)
//! └───────────────────────────────── prefix (your namespace)
//! ```
//!
//! The hash is **content-addressed**: your title, description, creator and
//! creation timestamp are length-prefixed, SHA-256'd, and rendered in base36.
//! Identical content at an identical instant yields an identical ID — the
//! timestamp is what keeps two same-titled records apart.
//!
//! Its **length adapts** to how many IDs you already have, via a birthday
//! bound: a young dataset gets three characters, a dataset of a million has
//! silently widened to eight. You never pick a length.
//!
//! Uniqueness is not left to the hash. Every candidate is checked against your
//! store, and on collision the minter escalates — more nonces, then a longer
//! hash, then a wider fallback. If it genuinely runs out it returns
//! [`IdError::Exhausted`]; it will not hand back an ID it knows is taken.
//!
//! # What is deliberately not here
//!
//! **Resolution** — turning a typed fragment like `8cd` back into
//! `myproj-8cda` — lives in the `hashid-resolve` crate. It is a different job
//! with a different shape: minting asks *is this taken?*, which any store
//! answers with an indexed lookup, while resolution needs a search. Keeping
//! them apart means a caller who only mints never meets the search.
//!
//! **Anything derivable from [`parse_id`] in a line.** [`Id`] carries
//! [`prefix`](Id::prefix), [`hash`](Id::hash) and
//! [`child_path`](Id::child_path), so an ID's depth, its root-ness, its parent
//! and its n-th child are each one expression in your own code. An export you
//! have to look up costs more than the line it saves.
//!
//! # Architecture
//!
//! ```text
//! domain/    Id · Prefix · Slug · Base36Hash · HashLength · IdConfig · Request
//!            correct by construction — you cannot hold a malformed one
//! generate/  seed → digest → length → escalation ladder
//! parse/     the trust boundary: text in, validated Id out
//! store.rs   StoreError — the one thing the crate needs from you
//! ```
//!
//! The crate performs no I/O and has no async runtime, no storage engine, and
//! no `unsafe`. It ships no store: yours is the only thing that knows where the
//! IDs really live.

/// The README's examples are compiled and run as doctests, so they cannot rot.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct Readme;

pub mod domain;
pub mod error;
pub mod generate;
pub mod parse;
pub mod store;

pub use domain::config::IdConfig;
pub use domain::hash::Base36Hash;
pub use domain::id::Id;
pub use domain::length::HashLength;
pub use domain::limits::{MAX_HASH_LEN, MAX_ID_LEN, MAX_PREFIX_LEN, MAX_SLUG_LEN};
pub use domain::prefix::Prefix;
pub use domain::request::Request;
pub use domain::slug::Slug;
pub use error::{IdError, MalformedReason, Result};
pub use generate::{base36, digest, seed, Minter};
pub use parse::{is_valid_id, parse_id, validate_prefix};
pub use store::StoreError;
