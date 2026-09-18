//! Turn what a human typed back into the ID they meant.
//!
//! Short IDs exist to be retyped, and nobody retypes the whole thing. Given
//! `br-8cda9f21`, a human types `8cda`. This crate turns that back.
//!
//! ```
//! use hashid::Id;
//! use hashid_resolve::{MatchType, Resolver};
//!
//! let known: Vec<Id> = ["br-abc123", "br-xyz789"]
//!     .iter()
//!     .map(|raw| Id::parse(raw))
//!     .collect::<Result<_, _>>()?;
//!
//! let resolver: Resolver = Resolver::new("br")?;
//!
//! let resolved = resolver.resolve(
//!     "xyz",
//!     |id| Ok(known.contains(id)),
//!     |_fragment| Ok(known.clone()),
//! )?;
//!
//! assert_eq!(resolved.id.to_string(), "br-xyz789");
//! assert_eq!(resolved.match_type, MatchType::Substring);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! # The two lookups
//!
//! `exists` is an indexed point lookup — `select 1 from ids where id = ?`.
//!
//! `candidates` is a **best-effort narrowing**: return anything that might
//! match and this crate applies [`matches_fragment`] to what comes back. Over-
//! returning costs only time, so every one of these is correct:
//!
//! ```sql
//! select id from ids where hash like '%' || ? || '%'   -- has a hash column
//! select id from ids where id   like '%' || ? || '%'   -- has only the id text
//! select id from ids                                    -- small table
//! ```
//!
//! Under-returning is the only way to get a wrong answer, and it is the one
//! thing a store cannot be forgiven for: a candidate you omit is a match this
//! crate will never see.
//!
//! # Why this is a separate crate
//!
//! Minting asks *is this ID taken?*, which any store answers with an indexed
//! lookup. Resolution needs a search. Keeping them apart means a caller who
//! only mints — the common case, and what [`hashid`] is named after — never
//! meets the search.

/// The README's examples are compiled and run as doctests, so they cannot rot.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct Readme;

pub mod config;
pub mod error;
pub mod matcher;
pub mod resolved;
pub mod resolver;

pub use config::ResolverConfig;
pub use error::{ResolveError, Result};
pub use matcher::{find_matching, matches_fragment};
pub use resolved::{MatchType, ResolvedId};
pub use resolver::{Candidates, Exists, Resolver};
