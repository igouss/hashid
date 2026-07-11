//! Content-addressed, human-typeable short IDs.
//!
//! `hashid` mints identifiers like `br-8cda`, `myproj-survey-the-users-3f9k1`,
//! or `br-8cda.1.2` — short enough to retype from a terminal, stable enough to
//! put in a commit message, and unique enough to key a database on.
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
//! The hash is **content-addressed**: your title, description, creator, and
//! creation timestamp are length-prefixed, SHA-256'd, and rendered in base36.
//! Identical content at an identical instant yields an identical ID — the
//! timestamp is what keeps two same-titled records apart.
//!
//! Its **length adapts** to how many IDs you already have, via a birthday
//! bound: a young dataset gets three characters, a dataset of a million has
//! silently widened to eight. You never pick a length.
//!
//! Uniqueness is not left to the hash. Every candidate is checked against your
//! store through the [`IdRegistry`] port, and on collision the generator
//! escalates — more nonces, then a longer hash, then a wider fallback. If it
//! genuinely runs out it returns [`IdError::Exhausted`]; it will not hand back
//! an ID it knows is taken.
//!
//! # Minting
//!
//! ```
//! use chrono::Utc;
//! use hashid::{IdConfig, IdGenerator, IdRequest, InMemoryIds, Prefix};
//!
//! let generator = IdGenerator::new(IdConfig::new(Prefix::normalize("myproj")?));
//! let mut known = InMemoryIds::new();
//!
//! let request = IdRequest::new("Ship the thing", Utc::now(), known.len())
//!     .with_creator("iouri");
//! let id = generator.mint(&request, &known)?;
//!
//! assert_eq!(id.prefix().as_str(), "myproj");
//! known.insert(id);
//! # Ok::<(), hashid::IdError>(())
//! ```
//!
//! # Resolving what a human typed
//!
//! Short IDs exist to be retyped, and nobody retypes the whole thing.
//!
//! ```
//! use hashid::{Id, IdResolver, InMemoryIds, MatchType, Prefix, ResolverConfig};
//!
//! let known: InMemoryIds = ["br-abc123", "br-xyz789"]
//!     .iter()
//!     .map(|raw| Id::parse(raw))
//!     .collect::<Result<InMemoryIds, _>>()?;
//!
//! let resolver = IdResolver::new(ResolverConfig::new(Prefix::parse("br")?));
//!
//! assert_eq!(resolver.resolve("xyz", &known)?.id.to_string(), "br-xyz789");
//! assert_eq!(resolver.resolve("abc123", &known)?.match_type, MatchType::PrefixNormalized);
//! assert!(resolver.resolve("zzz", &known).is_err());
//! # Ok::<(), hashid::IdError>(())
//! ```
//!
//! # Architecture
//!
//! Hexagonal, dependencies pointing inward:
//!
//! - [`domain`] — [`Id`], [`Prefix`], [`Slug`], [`Base36Hash`], [`IdConfig`].
//!   Correct by construction; you cannot hold a malformed one.
//! - [`generate`], [`parse`], [`resolve`] — one capability each.
//! - [`port`] — [`IdRegistry`] and [`IdIndex`], the two questions the crate
//!   asks of your store. Both fallible: an unreachable database and an absent
//!   ID are different answers.
//! - [`adapter`] — [`InMemoryIds`] and closure adapters, so you can use the
//!   crate without implementing a trait.
//!
//! The crate performs no I/O and has no async runtime, no storage engine, and
//! no `unsafe`.
//!
//! [`Base36Hash`]: domain::hash::Base36Hash

/// The README's examples are compiled and run as doctests, so they cannot rot.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct Readme;

pub mod adapter;
pub mod domain;
pub mod error;
pub mod generate;
pub mod parse;
pub mod port;
pub mod resolve;

pub use adapter::{FnIndex, FnRegistry, InMemoryIds};
pub use domain::config::IdConfig;
pub use domain::hash::Base36Hash;
pub use domain::id::Id;
pub use domain::limits::{MAX_HASH_LEN, MAX_ID_LEN, MAX_PREFIX_LEN, MAX_SLUG_LEN};
pub use domain::prefix::Prefix;
pub use domain::request::IdRequest;
pub use domain::resolved::{MatchType, ResolvedId};
pub use domain::slug::Slug;
pub use error::{IdError, MalformedReason, Result};
pub use generate::IdGenerator;
pub use parse::{is_valid_id, parse_id, validate_prefix};
pub use port::{IdIndex, IdRegistry, RegistryError};
pub use resolve::{find_matching, matches_fragment, IdResolver, ResolverConfig};
